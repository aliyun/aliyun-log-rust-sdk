"""Release matrix and artifact validation; run with Python 3.12 and packaging."""

import argparse
from email.parser import BytesParser
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tomllib
import zipfile

from packaging.utils import canonicalize_name, parse_wheel_filename
from packaging.version import Version


# Tier 1 gets version-specific wheels; every platform also gets cp38-abi3.
PYTHONS = ["3.10", "3.11", "3.12", "3.13", "3.14"]
LINUX = [
    ("manylinux2014", "x86_64", "x86_64-unknown-linux-gnu", True),
    ("manylinux2014", "aarch64", "aarch64-unknown-linux-gnu", True),
    ("manylinux2014", "i686", "i686-unknown-linux-gnu", False),
    ("manylinux2014", "ppc64le", "powerpc64le-unknown-linux-gnu", False),
    ("manylinux2014", "s390x", "s390x-unknown-linux-gnu", False),
    ("manylinux_2_31", "armv7l", "armv7-unknown-linux-gnueabihf", False),
    ("manylinux_2_39", "riscv64", "riscv64gc-unknown-linux-gnu", False),
    ("musllinux_1_2", "x86_64", "x86_64-unknown-linux-musl", True),
    ("musllinux_1_2", "aarch64", "aarch64-unknown-linux-musl", True),
    ("musllinux_1_2", "i686", "i686-unknown-linux-musl", False),
    ("musllinux_1_2", "armv7l", "armv7-unknown-linux-musleabihf", False),
]
DESKTOP = [
    ("macos-15-intel", "x86_64-apple-darwin", "x64", "10.13", False),
    ("macos-15", "aarch64-apple-darwin", "arm64", "11.0", True),
    ("windows-2022", "x86_64-pc-windows-msvc", "x64", "", True),
    ("windows-2022", "i686-pc-windows-msvc", "x86", "", False),
    # Cross-build the ARM64 ABI3 fallback using an x64 host interpreter.
    ("windows-2022", "aarch64-pc-windows-msvc", "x64", "", False),
]


def version():
    with Path("bindings/python/Cargo.toml").open("rb") as source:
        return Version(tomllib.load(source)["package"]["version"])


def build_matrices():
    linux = []
    for policy, arch, target, native in LINUX:
        for kind, python in [("abi3", "3.12")] + ([("native", p) for p in PYTHONS] if native else []):
            linux.append(dict(
                id="{}-{}-{}-{}".format(policy, arch, kind, python),
                policy=policy, target=target, python=python,
                container="quay.io/pypa/{}_{}:latest".format(policy, arch),
                features="--features vendored-openssl" + (" --no-default-features" if kind == "native" else ""),
            ))
    desktop = []
    for runner, target, arch, deployment, native in DESKTOP:
        for kind, python in [("abi3", "3.12")] + ([("native", p) for p in PYTHONS] if native else []):
            desktop.append(dict(
                id="{}-{}-{}".format(target, kind, python), runner=runner,
                target=target, arch=arch, deployment=deployment, python=python,
                features="--no-default-features" if kind == "native" else "",
            ))
    return linux, desktop


def status_targets():
    targets = {}
    for matrix in build_matrices():
        for entry in matrix:
            if entry["id"].rsplit("-", 2)[1] != "abi3":
                continue
            platform = entry["id"].rsplit("-", 2)[0]
            targets.setdefault(platform, []).append("wheel / " + entry["id"])
    return targets


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


def check_metadata(data, expected):
    metadata = BytesParser().parsebytes(data)
    if canonicalize_name(metadata["Name"]) != "aliyun-log-producer":
        raise ValueError("unexpected distribution name")
    if Version(metadata["Version"]) != expected or metadata["Requires-Python"] != ">=3.8":
        raise ValueError("unexpected package version or Python requirement")


def check_dist(directory, expected_version, expected_wheels):
    expected = Version(expected_version)
    wheels = sorted(directory.glob("*.whl"))
    sources = list(directory.glob("*.tar.gz"))
    if len(wheels) != expected_wheels or len(sources) != 1:
        raise ValueError("expected {} wheels and one sdist, found {} and {}".format(
            expected_wheels, len(wheels), len(sources)))
    for wheel in wheels:
        name, wheel_version, _, tags = parse_wheel_filename(wheel.name)
        if name != "aliyun-log-producer" or wheel_version != expected:
            raise ValueError("unexpected wheel identity: {}".format(wheel.name))
        with zipfile.ZipFile(wheel) as archive:
            metadata = [p for p in archive.namelist() if p.endswith(".dist-info/METADATA")]
            if len(metadata) != 1:
                raise ValueError("expected one METADATA file")
            check_metadata(archive.read(metadata[0]), expected)
        if any(tag.abi == "abi3" for tag in tags):
            if any(tag.interpreter != "cp38" or tag.abi != "abi3" for tag in tags):
                raise ValueError("expected cp38-abi3 fallback")
            subprocess.run([sys.executable, "-m", "abi3audit", "--strict", str(wheel)], check=True)
    with tarfile.open(sources[0]) as archive:
        metadata = [m for m in archive.getmembers() if m.name.endswith("/PKG-INFO") and m.name.count("/") == 1]
        if len(metadata) != 1:
            raise ValueError("expected one sdist PKG-INFO file")
        check_metadata(archive.extractfile(metadata[0]).read(), expected)
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
