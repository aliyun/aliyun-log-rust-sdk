"""Check a wheel before auditing it or installing it for runtime tests."""

import argparse
from email.parser import BytesParser
import hashlib
from pathlib import Path
import subprocess
import sys
import zipfile

from platforms import ABI3_PYTHON, ABI3_TAG


def read_metadata(data, expected_version=None):
    metadata = BytesParser().parsebytes(data)
    if metadata["Name"] != "aliyun-log-producer" or not metadata["Version"]:
        raise ValueError("unexpected package identity")
    if expected_version is not None and metadata["Version"] != expected_version:
        raise ValueError("unexpected package version")
    if metadata["Requires-Python"] != ">=" + ABI3_PYTHON:
        raise ValueError("expected Requires-Python: >=" + ABI3_PYTHON)
    return metadata


def read_wheel(wheel, expected_version=None):
    with zipfile.ZipFile(wheel) as archive:
        wheel_info = [name for name in archive.namelist() if name.endswith(".dist-info/WHEEL")]
        package_info = [name for name in archive.namelist() if name.endswith(".dist-info/METADATA")]
        if len(wheel_info) != 1 or len(package_info) != 1:
            raise ValueError("expected one WHEEL and one METADATA file")
        metadata = read_metadata(archive.read(package_info[0]), expected_version)
        tags = BytesParser().parsebytes(archive.read(wheel_info[0])).get_all("Tag", [])
        if not tags:
            raise ValueError("missing WHEEL tags")
    return metadata, tags


def wheel_prefix(kind):
    return ABI3_TAG + "-" if kind == "abi3" else "cp{0}{1}-cp{0}{1}-".format(*sys.version_info[:2])


def inspect_wheel(wheel, kind):
    prefix = wheel_prefix(kind)
    if "-" + prefix not in wheel.name:
        raise ValueError("expected a {} wheel: {}".format(prefix, wheel.name))
    metadata, tags = read_wheel(wheel)
    if any(not tag.startswith(prefix) for tag in tags):
        raise ValueError("unexpected WHEEL tags: {}".format(tags))
    print("{} sha256={}".format(wheel.name, hashlib.sha256(wheel.read_bytes()).hexdigest()), flush=True)
    return metadata["Name"], metadata["Version"]


def one_wheel(directory):
    wheels = list(directory.glob("*.whl"))
    if len(wheels) != 1:
        raise ValueError("expected exactly one wheel, found {}".format(len(wheels)))
    return wheels[0].resolve()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheel_directory", type=Path)
    parser.add_argument("--kind", choices=["abi3", "native"], default="abi3")
    parser.add_argument("--audit", action="store_true")
    parser.add_argument("--install", action="store_true")
    args = parser.parse_args()
    if args.audit and args.kind != "abi3":
        parser.error("stable ABI auditing applies only to ABI3 wheels")
    try:
        wheel = one_wheel(args.wheel_directory)
        inspect_wheel(wheel, args.kind)
    except ValueError as error:
        parser.error(str(error))
    if args.audit:
        subprocess.run([sys.executable, "-m", "abi3audit", "--strict", str(wheel)], check=True)
    if args.install:
        # Reinstall even at the same version: native and ABI3 are alternative
        # builds of one distribution. Otherwise pip may keep the previous build.
        subprocess.run([
            sys.executable, "-m", "pip", "install", "--only-binary=:all:",
            "--force-reinstall", "{}[test]".format(wheel),
        ], check=True)
        subprocess.run([
            sys.executable, "-c",
            "import sys; from aliyun_log_producer import _native; "
            "print(sys.version); print(_native.__file__)",
        ], check=True)


if __name__ == "__main__":
    main()
