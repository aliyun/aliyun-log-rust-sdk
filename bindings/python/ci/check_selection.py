"""Verify binary-only pip selection, including CPython native/ABI3 fallback."""

import argparse
from pathlib import Path
import subprocess
import sys
import tempfile

from check_wheel import inspect_wheel, one_wheel, wheel_prefix


def check_selection(directories, expected, requirement, kind):
    with tempfile.TemporaryDirectory() as temporary:
        destination = Path(temporary)
        command = [
            sys.executable, "-m", "pip", "download", "--no-index",
            "--no-cache-dir", "--only-binary=:all:", "--no-deps",
            "--dest", str(destination),
        ]
        for directory in directories:
            command.extend(["--find-links", str(directory.resolve())])
        subprocess.run(command + [requirement], check=True)
        selected = one_wheel(destination)
        if selected.name != expected.name or selected.read_bytes() != expected.read_bytes():
            raise RuntimeError("pip selected an unexpected artifact: {}".format(selected.name))
        # Exercise switching between same-version builds and import in a fresh
        # process, so a cached module or stale native extension cannot mask it.
        subprocess.run([
            sys.executable, "-m", "pip", "install", "--no-index", "--no-deps",
            "--force-reinstall", str(selected),
        ], check=True)
        prefix = wheel_prefix(kind)
        subprocess.run([
            sys.executable, "-c",
            "from importlib.metadata import distribution; "
            "from email.parser import Parser; "
            "from aliyun_log_producer import _native; "
            "d = distribution('aliyun-log-producer'); "
            "tags = Parser().parsestr(d.read_text('WHEEL')).get_all('Tag', []); "
            "assert tags and all(t.startswith({!r}) for t in tags), tags; "
            "print('Imported:', _native.__file__)".format(prefix),
        ], check=True)
        print("{} selection passed: {}".format(kind, selected.name), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("abi3_directory", type=Path)
    parser.add_argument("native_directory", type=Path, nargs="?")
    parser.add_argument("--kind", choices=["abi3", "pypy", "graalpy", "free-threaded"], default="abi3")
    args = parser.parse_args()
    if args.kind != "abi3":
        if args.native_directory is not None:
            parser.error("alternate runtimes do not have an ABI3 fallback")
        wheel = one_wheel(args.abi3_directory)
        name, version = inspect_wheel(wheel, args.kind)
        check_selection([args.abi3_directory], wheel, "{}=={}".format(name, version), args.kind)
        return
    try:
        abi3 = one_wheel(args.abi3_directory)
        name, version = inspect_wheel(abi3, "abi3")
        if args.native_directory is not None:
            native = one_wheel(args.native_directory)
            if inspect_wheel(native, "native") != (name, version):
                parser.error("both builds must have the same package name and version")
    except ValueError as error:
        parser.error(str(error))
    requirement = "{}=={}".format(name, version)
    if args.native_directory is not None:
        check_selection([args.abi3_directory, args.native_directory], native, requirement, "native")
    check_selection([args.abi3_directory], abi3, requirement, "abi3")


if __name__ == "__main__":
    main()
