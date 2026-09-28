"""Shared platform and Python-version policy for build workflows."""

import json
import os

ABI3_PYTHON = "3.8"
ABI3_TAG = "cp" + ABI3_PYTHON.replace(".", "") + "-abi3"
BUILD_PYTHON = "3.12"
ABI3_ONLY_PYTHONS = [ABI3_PYTHON, "3.9"]
# Platforms with native=True get version-specific wheels.
NATIVE_PYTHONS = ["3.10", "3.11", "3.12", "3.13", "3.14"]
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


def build_matrices():
    linux = []
    for policy, arch, target, native in LINUX:
        for kind, python in [("abi3", BUILD_PYTHON)] + ([("native", p) for p in NATIVE_PYTHONS] if native else []):
            linux.append(dict(
                id="{}-{}-{}-{}".format(policy, arch, kind, python),
                platform="{}-{}".format(policy, arch), kind=kind,
                policy=policy, target=target, python=python,
                container="quay.io/pypa/{}_{}:latest".format(policy, arch),
                features="--features vendored-openssl" + (" --no-default-features" if kind == "native" else ""),
            ))
    desktop = []
    for runner, target, arch, deployment, native in DESKTOP:
        for kind, python in [("abi3", BUILD_PYTHON)] + ([("native", p) for p in NATIVE_PYTHONS] if native else []):
            desktop.append(dict(
                id="{}-{}-{}".format(target, kind, python),
                platform=target, kind=kind, runner=runner,
                target=target, arch=arch, deployment=deployment, python=python,
                features="--no-default-features" if kind == "native" else "",
            ))
    return linux, desktop


def status_targets():
    targets = {}
    for matrix in build_matrices():
        for entry in matrix:
            if entry["kind"] != "abi3":
                continue
            platform = entry["platform"]
            targets.setdefault(platform, []).append("wheel / " + entry["id"])
    return targets


def ci_matrices():
    # Keep the existing runtime test hosts; this does not add platform coverage.
    hosts = [
        ("ubuntu-22.04", "x86_64-unknown-linux-gnu"),
        ("macos-15-intel", "x86_64-apple-darwin"),
        ("windows-2022", "x86_64-pc-windows-msvc"),
    ]
    native_targets = {
        entry["target"]
        for matrix in build_matrices() for entry in matrix
        if entry["kind"] == "native"
    }
    return {
        "abi3": {"include": [{"os": runner} for runner, _ in hosts]},
        "runtime": {"include": [
            {"os": runner, "python": python,
             "native": target in native_targets and python in NATIVE_PYTHONS}
            for runner, target in hosts
            for python in ABI3_ONLY_PYTHONS + NATIVE_PYTHONS
        ]},
    }


if __name__ == "__main__":
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for name, matrix in ci_matrices().items():
            output.write("{}={}\n".format(name, json.dumps(matrix)))
