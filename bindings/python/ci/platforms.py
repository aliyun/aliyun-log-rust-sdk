"""Release policy: pydantic-core 2.46.5 platforms, CPython ABI3, GraalPy Python 3.12/3.13."""

import json
import os

ABI3_PYTHON = "3.8"
ABI3_TAG = "cp38-abi3"
BUILD_PYTHON = "3.12"
# Latest verified release for each Python language version.
GRAALPY_RELEASES = {"3.12": "25.2.4", "3.13": "25.4.4"}
GRAALPY_ABIS = {"3.12": "250", "3.13": "253"}
# 25.0.1 is the last upstream macOS Intel distribution.
GRAALPY_MACOS_INTEL = "25.0.1"
# PyPy 7.3.23 implements Python 3.11.15; PyPy 8 changes the wheel ABI to pp80.
PYPY_BUILD_PYTHON = "3.11.15"
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
        version = python.replace(".", "")
        selector = "graalpy" + python
        tag = "graalpy{0}-graalpy{1}_{0}_native".format(version, GRAALPY_ABIS[python])
    else:
        selector = python
        version = python.rstrip("t").replace(".", "")
        tag = "cp{}-cp{}{}".format(version, version, "t" if python.endswith("t") else "")
    return dict(kind=kind, python=python, interpreter=selector, tag=tag,
                setup_python="graalpy-" + GRAALPY_RELEASES[python] if kind == "graalpy" else selector,
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
            variants.extend(("graalpy", version) for version in GRAALPY_RELEASES)
        for kind, python in variants:
            entry = runtime(kind, python)
            baseline = "manylinux_2_28" if kind == "graalpy" and python == "3.13" else policy
            entry.update(
                id="{}-{}-{}-{}".format(baseline, arch, kind, python),
                platform="{}-{}".format(baseline, arch), policy=baseline,
                target=target, wheel_platform="{}_{}".format(baseline, arch),
                features=(entry["features"] + " --features vendored-openssl").strip(),
            )
            linux.append(entry)
    for runner, target, arch, deployment in DESKTOP:
        versions = NATIVE_PYTHONS[2:] if target == "aarch64-pc-windows-msvc" else NATIVE_PYTHONS
        variants = [("abi3", BUILD_PYTHON)] + [("native", p) for p in versions] + [("free-threaded", "3.14t")]
        if "apple" in target or target == "x86_64-pc-windows-msvc":
            variants.append(("pypy", "3.11"))
        if "apple" in target:
            graalpy_versions = ["3.12"] if arch == "x64" else GRAALPY_RELEASES
            variants.extend(("graalpy", version) for version in graalpy_versions)
        if "apple" in target:
            wheel_platform = "macosx_{}_{}".format(deployment.replace(".", "_"), "arm64" if arch == "arm64" else "x86_64")
        else:
            wheel_platform = {"x64": "win_amd64", "x86": "win32", "arm64": "win_arm64"}[arch]
        for kind, python in variants:
            entry = runtime(kind, python)
            entry.update(id="{}-{}-{}".format(target, kind, python), platform=target,
                         runner=runner, target=target, arch=arch, deployment=deployment,
                         wheel_platform=wheel_platform)
            if kind == "graalpy" and arch == "x64":
                entry["setup_python"] = "graalpy-" + GRAALPY_MACOS_INTEL
            desktop.append(entry)
    return linux, desktop


def build_groups():
    matrices = []
    for matrix in build_matrices():
        groups = {}
        for entry in matrix:
            group_id = "{}-{}".format(entry["platform"], entry["kind"])
            if entry["kind"] == "graalpy":
                group_id += "-" + entry["python"]
            if group_id not in groups:
                groups[group_id] = {key: entry[key] for key in (
                    "platform", "kind", "target", "features", "policy", "runner", "arch", "deployment"
                ) if key in entry}
                groups[group_id].update(id=group_id, artifact_ids=[], interpreters=[], python_requests=[])
                if entry["kind"] == "graalpy" and "runner" in entry:
                    groups[group_id]["setup_python"] = entry["setup_python"]
            group = groups[group_id]
            group["artifact_ids"].append(entry["id"])
            group["interpreters"].append(entry["interpreter"])
            if "runner" in entry and entry["kind"] != "graalpy":
                system = "windows" if "windows" in entry["target"] else "darwin"
                arch = {"x64": "x86_64", "x86": "i686", "arm64": "aarch64"}[entry["arch"]]
                implementation = "pypy" if entry["kind"] == "pypy" else "cpython"
                python = PYPY_BUILD_PYTHON if entry["kind"] == "pypy" else entry["python"].replace("t", "+freethreaded")
                group["python_requests"].append("{}-{}-{}-{}-none".format(implementation, python, system, arch))
        for group in groups.values():
            group["interpreters"] = " ".join(group["interpreters"])
        matrices.append(list(groups.values()))
    return tuple(matrices)


def status_targets():
    targets = {}
    for matrix in build_groups():
        for group in matrix:
            key = group["platform"]
            if group["kind"] != "abi3":
                key += "-" + group["kind"]
            targets.setdefault(key, []).append("wheel / " + group["id"])
    return targets


def ci_matrices():
    hosts = ["ubuntu-22.04", "macos-15-intel", "macos-15", "windows-2022"]
    regular = [dict(os=host, python=python, native=python in NATIVE_PYTHONS)
               for host in hosts for python in ABI3_ONLY_PYTHONS + NATIVE_PYTHONS]
    alternative = []
    for host in hosts:
        for kind, python in ([("pypy", "3.11"), ("free-threaded", "3.14t")]
                             + [("graalpy", version) for version in GRAALPY_RELEASES]):
            if kind == "graalpy" and host.startswith("windows"):
                continue
            if kind == "graalpy" and host == "macos-15-intel" and python != "3.12":
                continue
            entry = runtime(kind, python)
            if kind == "graalpy" and host == "macos-15-intel":
                entry["setup_python"] = "graalpy-" + GRAALPY_MACOS_INTEL
            entry.update(os=host)
            alternative.append(entry)
    return {
        "abi3": {"include": [dict(os=host) for host in hosts]},
        "native": {"include": [entry for entry in regular if entry["native"]]},
        "runtime": {"include": regular},
        "alternative": {"include": alternative},
    }


if __name__ == "__main__":
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for name, matrix in ci_matrices().items():
            output.write("{}={}\n".format(name, json.dumps(matrix)))
