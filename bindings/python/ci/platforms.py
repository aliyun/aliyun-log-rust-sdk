"""Release policy: pydantic-core 2.46.5 platforms, CPython ABI3, GraalPy 25+."""

import json
import os

ABI3_PYTHON = "3.8"
ABI3_TAG = "cp38-abi3"
BUILD_PYTHON = "3.12"
ABI3_ONLY_PYTHONS = [ABI3_PYTHON]
NATIVE_PYTHONS = ["3.9", "3.10", "3.11", "3.12", "3.13", "3.14"]
# Let maturin-action select its maintained cross images, as pydantic-core does.
# i686 uses glibc 2.17: current Rust cannot honestly promise manylinux1 (2.5).
LINUX = [
    ("manylinux2014", "x86_64", "x86_64-unknown-linux-gnu", True),
    ("manylinux2014", "aarch64", "aarch64-unknown-linux-gnu", True),
    ("manylinux2014", "i686", "i686-unknown-linux-gnu", True),
    ("manylinux2014", "ppc64le", "powerpc64le-unknown-linux-gnu", True),
    ("manylinux2014", "s390x", "s390x-unknown-linux-gnu", True),
    ("manylinux2014", "armv7l", "armv7-unknown-linux-gnueabihf", True),
    ("manylinux_2_31", "riscv64", "riscv64gc-unknown-linux-gnu", True),
    ("musllinux_1_1", "x86_64", "x86_64-unknown-linux-musl", True),
    ("musllinux_1_1", "aarch64", "aarch64-unknown-linux-musl", True),
    ("musllinux_1_1", "i686", "i686-unknown-linux-musl", False),
    ("musllinux_1_1", "armv7l", "armv7-unknown-linux-musleabihf", True),
]
DESKTOP = [
    ("macos-15-intel", "x86_64-apple-darwin", "x64", "10.12"),
    ("macos-15", "aarch64-apple-darwin", "arm64", "11.0"),
    ("windows-2022", "x86_64-pc-windows-msvc", "x64", ""),
    ("windows-2022", "i686-pc-windows-msvc", "x86", ""),
    ("windows-11-arm", "aarch64-pc-windows-msvc", "arm64", ""),
]
PYPY_LINUX = {
    ("manylinux2014", "x86_64"), ("manylinux2014", "i686"),
    ("musllinux_1_1", "x86_64"), ("musllinux_1_1", "aarch64"),
    ("musllinux_1_1", "armv7l"),
}
GRAALPY_LINUX = {("manylinux2014", "x86_64"), ("manylinux2014", "aarch64")}


def runtime(kind, python):
    """Keep build selector, implementation and expected wheel ABI independent."""
    if kind == "abi3":
        selector, tag = BUILD_PYTHON, ABI3_TAG
    elif kind == "pypy":
        selector, tag = "pypy3.11", "pp311-pypy311_pp73"
    elif kind == "graalpy":
        selector, tag = "graalpy3.12", "graalpy312-graalpy250_312_native"
    else:
        selector = python
        version = python.rstrip("t").replace(".", "")
        tag = "cp{}-cp{}{}".format(version, version, "t" if python.endswith("t") else "")
    return dict(kind=kind, python=python, interpreter=selector, tag=tag,
                features="" if kind == "abi3" else "--no-default-features")


def build_matrices():
    linux, desktop = [], []
    for policy, arch, target, native in LINUX:
        variants = [("abi3", BUILD_PYTHON)]
        if native:
            variants += [("native", p) for p in NATIVE_PYTHONS] + [("free-threaded", "3.14t")]
        if (policy, arch) in PYPY_LINUX:
            variants.append(("pypy", "3.11"))
        if (policy, arch) in GRAALPY_LINUX:
            variants.append(("graalpy", "3.12"))
        for kind, python in variants:
            entry = runtime(kind, python)
            entry.update(
                id="{}-{}-{}-{}".format(policy, arch, kind, python),
                platform="{}-{}".format(policy, arch), policy=policy,
                target=target, wheel_platform="{}_{}".format(policy, arch),
                features=(entry["features"] + " --features vendored-openssl").strip(),
            )
            linux.append(entry)
    for runner, target, arch, deployment in DESKTOP:
        versions = NATIVE_PYTHONS[2:] if target == "aarch64-pc-windows-msvc" else NATIVE_PYTHONS
        variants = [("abi3", BUILD_PYTHON)] + [("native", p) for p in versions] + [("free-threaded", "3.14t")]
        if "apple" in target or target == "x86_64-pc-windows-msvc":
            variants.append(("pypy", "3.11"))
        if "apple" in target:
            variants.append(("graalpy", "3.12"))
        if "apple" in target:
            wheel_platform = "macosx_{}_{}".format(deployment.replace(".", "_"), "arm64" if arch == "arm64" else "x86_64")
        else:
            wheel_platform = {"x64": "win_amd64", "x86": "win32", "arm64": "win_arm64"}[arch]
        for kind, python in variants:
            entry = runtime(kind, python)
            entry.update(id="{}-{}-{}".format(target, kind, python), platform=target,
                         runner=runner, target=target, arch=arch, deployment=deployment,
                         wheel_platform=wheel_platform)
            desktop.append(entry)
    return linux, desktop


def status_targets():
    targets = {}
    for matrix in build_matrices():
        for entry in matrix:
            # Preserve ABI3 badge names; give the other runtimes separate badges.
            key = entry["platform"]
            if entry["kind"] != "abi3":
                key += "-" + entry["kind"]
            targets.setdefault(key, []).append("wheel / " + entry["id"])
    return targets


def ci_matrices():
    hosts = ["ubuntu-22.04", "macos-15-intel", "macos-15", "windows-2022"]
    regular = [dict(os=host, python=python, native=python in NATIVE_PYTHONS)
               for host in hosts for python in ABI3_ONLY_PYTHONS + NATIVE_PYTHONS]
    alternative = []
    for host in hosts:
        for kind, python in [("pypy", "3.11"), ("graalpy", "3.12"), ("free-threaded", "3.14t")]:
            if kind == "graalpy" and host.startswith("windows"):
                continue
            entry = runtime(kind, python)
            # setup-python specifies GraalVM release, rather than Python version.
            entry.update(os=host, setup_python="graalpy-25.0" if kind == "graalpy" else entry["interpreter"])
            alternative.append(entry)
    return {
        "abi3": {"include": [dict(os=host) for host in hosts]},
        "runtime": {"include": regular},
        "alternative": {"include": alternative},
    }


if __name__ == "__main__":
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for name, matrix in ci_matrices().items():
            output.write("{}={}\n".format(name, json.dumps(matrix)))
