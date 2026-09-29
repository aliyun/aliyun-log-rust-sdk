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
GRAALPY_LINUX_SHA256 = {
    ("3.12", "x86_64"): "b3d0766ae6d55daa15f0db1f6c884383d7eec506b186d0ba2f702523a78ff28d",
    ("3.12", "aarch64"): "e57472272b1b659ae6ac972117723b0515a49cc9577688ed5563919793170d67",
    ("3.13", "x86_64"): "8b72e6e513d06976e5c8e051228d69d541fde76d05d5f907eaef4bb31db83af4",
    ("3.13", "aarch64"): "1323dc064583efd1d6b696d3a9f9e4b1ff8facd0f7cc2b69fe0a0bf7ef06ea1a",
}
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
            entry.update(
                id="{}-{}-{}-{}".format(policy, arch, kind, python),
                platform="{}-{}".format(policy, arch), policy=policy,
                target=target, wheel_platform="{}_{}".format(policy, arch),
                features=(entry["features"] + " --features vendored-openssl").strip(),
            )
            if kind == "graalpy":
                release = GRAALPY_RELEASES[python]
                machine = "amd64" if arch == "x86_64" else arch
                entry.update(
                    interpreter="/opt/sls-graalpy/bin/graalpy",
                    graalpy_url="https://github.com/oracle/graalpython/releases/download/graal-{0}/graalpy{1}-{0}-linux-{2}.tar.gz".format(release, python, machine),
                    graalpy_sha256=GRAALPY_LINUX_SHA256[python, arch],
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
        "runtime": {"include": regular},
        "alternative": {"include": alternative},
    }


if __name__ == "__main__":
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        for name, matrix in ci_matrices().items():
            output.write("{}={}\n".format(name, json.dumps(matrix)))
