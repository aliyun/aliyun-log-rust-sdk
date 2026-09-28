"""Release matrix and artifact validation; run with Python 3.12 and packaging."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tomllib

from packaging.tags import parse_tag
from packaging.utils import parse_wheel_filename
from packaging.version import Version

from check_wheel import read_metadata, read_wheel
from platforms import ABI3_TAG, build_matrices, status_targets


def version():
    with Path("bindings/python/Cargo.toml").open("rb") as source:
        return Version(tomllib.load(source)["package"]["version"])


def prepare():
    package_version = version()
    draft_tag = os.environ.get("DRAFT_VERSION", "").strip()
    tag = draft_tag or (os.environ.get("GITHUB_REF_NAME", "")
                        if os.environ.get("GITHUB_EVENT_NAME") == "push"
                        and os.environ.get("GITHUB_REF_TYPE") == "tag" else "")
    if tag:
        if not re.fullmatch(r"python-v[0-9A-Za-z.+-]+", tag):
            raise ValueError("use a python-v<VERSION> tag")
        if Version(tag.removeprefix("python-v")) != package_version:
            raise ValueError("release tag must match bindings/python/Cargo.toml; no automatic version rewrite")
        existing = subprocess.run(
            ["git", "rev-parse", "--verify", "--quiet", "refs/tags/{}^{{commit}}".format(tag)],
            capture_output=True, text=True,
        )
        head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
        if existing.returncode == 0 and existing.stdout.strip() != head:
            raise ValueError("release tag points at a different commit from the build checkout")
    linux, desktop = build_matrices()
    outputs = dict(
        version=str(package_version), tag=tag, draft=str(bool(draft_tag)).lower(),
        prerelease=str(package_version.is_prerelease or package_version.is_devrelease).lower(),
        linux=json.dumps({"include": linux}), desktop=json.dumps({"include": desktop}),
        wheel_count=str(len(linux) + len(desktop)),
    )
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for key, value in outputs.items():
            output.write("{}={}\n".format(key, value))
    print("Release {}: {} Linux + {} desktop wheels; draft={}".format(
        package_version, len(linux), len(desktop), bool(draft_tag)))


def check_coverage(wheels):
    """Require exactly one artifact per matrix entry, including ABI and baseline."""
    entries = [entry for matrix in build_matrices() for entry in matrix]
    remaining = {entry["id"] for entry in entries}
    for wheel in wheels:
        tags = parse_wheel_filename(wheel.name)[3]
        matches = [entry for entry in entries if any(
            "{}-{}".format(tag.interpreter, tag.abi) == entry["tag"]
            and tag.platform == entry["wheel_platform"] for tag in tags)]
        if len(matches) != 1:
            raise ValueError("unexpected ABI/platform/baseline: {}".format(wheel.name))
        entry = matches[0]
        if entry["id"] not in remaining:
            raise ValueError("duplicate build: {}".format(entry["id"]))
        remaining.remove(entry["id"])
    if remaining:
        raise ValueError("missing builds: {}".format(", ".join(sorted(remaining))))


def check_dist(directory, expected_version, expected_wheels):
    expected = Version(expected_version)
    wheels = sorted(directory.glob("*.whl"))
    sources = list(directory.glob("*.tar.gz"))
    if len(wheels) != expected_wheels or len(sources) != 1:
        raise ValueError("expected {} wheels and one sdist, found {} and {}".format(
            expected_wheels, len(wheels), len(sources)))
    check_coverage(wheels)
    for wheel in wheels:
        name, wheel_version, _, tags = parse_wheel_filename(wheel.name)
        if name != "aliyun-log-producer" or wheel_version != expected:
            raise ValueError("unexpected wheel identity: {}".format(wheel.name))
        _, wheel_tags = read_wheel(wheel, str(expected))
        if {tag for value in wheel_tags for tag in parse_tag(value)} != tags:
            raise ValueError("WHEEL tags do not match filename: {}".format(wheel.name))
        if any(tag.abi == "abi3" for tag in tags):
            if any("{}-{}".format(tag.interpreter, tag.abi) != ABI3_TAG for tag in tags):
                raise ValueError("expected {} fallback".format(ABI3_TAG))
            subprocess.run([sys.executable, "-m", "abi3audit", "--strict", str(wheel)], check=True)
    with tarfile.open(sources[0]) as archive:
        metadata = [m for m in archive.getmembers() if m.name.endswith("/PKG-INFO") and m.name.count("/") == 1]
        if len(metadata) != 1:
            raise ValueError("expected one sdist PKG-INFO file")
        read_metadata(archive.extractfile(metadata[0]).read(), str(expected))
    with (directory / "SHA256SUMS").open("w") as checksums:
        for path in sorted(wheels + sources):
            checksums.write("{}  {}\n".format(hashlib.sha256(path.read_bytes()).hexdigest(), path.name))
    print("Validated {} wheels and one sdist".format(len(wheels)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("prepare")
    commands.add_parser("status-targets")
    check = commands.add_parser("check-dist")
    check.add_argument("directory", type=Path)
    check.add_argument("--version", required=True)
    check.add_argument("--wheels", type=int, required=True)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare()
    elif args.command == "status-targets":
        print(json.dumps(status_targets()))
    else:
        check_dist(args.directory, args.version, args.wheels)


if __name__ == "__main__":
    main()
