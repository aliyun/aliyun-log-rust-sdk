# Python build and release notes

[User guide](../README.md)

## Platforms and Python versions

The package targets the main interpreter of ordinary **CPython 3.8–3.14**.
Version-specific wheels preserve CPython-specific optimizations; `cp38-abi3`
wheels provide installation fallback for compatible CPython 3.8+ runtimes.
Later CPython versions may accept the stable ABI wheel, but still need runtime
validation. Python 2/3.7 and older, PyPy, GraalPy, free-threaded Python,
subinterpreters and reusing an inherited producer after fork are outside the
current support contract. Create a new producer in each child process.

The following is the **configured release target matrix**, not a claim that the
new release workflow has already passed on every target. macOS ARM64 has local
runtime coverage on CPython 3.8–3.14; the release matrix awaits its first CI run.
Only advertise a downloadable wheel once it is present in that release.

| Platform | Architecture | ABI3 fallback | Version-specific wheels |
| --- | --- | --- | --- |
| Linux, glibc 2.17+ (`manylinux2014`) | x86_64, aarch64 | CPython 3.8+ | 3.9–3.14 |
| Linux, musl 1.2+ (`musllinux_1_2`, including compatible Alpine) | x86_64, aarch64 | CPython 3.8+ | 3.9–3.14 |
| macOS, deployment target 10.13 | Intel x86_64 | CPython 3.8+ | 3.8–3.14 |
| macOS 11+ | Apple Silicon ARM64 | CPython 3.8+ | 3.8–3.14 |
| Windows, modern MSVC runtime | x86_64 | CPython 3.8+ | 3.8–3.14 |

Extended architectures are configured for **ABI3-only builds** and have not
received runtime validation:

| Platform | Architecture | Build baseline |
| --- | --- | --- |
| Linux / glibc | i686, ppc64le, s390x | glibc 2.17+ |
| Linux / glibc | ARMv7 hard-float | glibc 2.31+ |
| Linux / glibc | RISC-V 64 | glibc 2.39+ |
| Linux / musl | i686, ARMv7 hard-float | musl 1.2+ |
| Windows | x86 (32-bit), ARM64 | MSVC; ARM64 is cross-built |

The installed Python itself can require a newer OS than the wheel baseline.
Both requirements apply. Intel and ARM64 macOS wheels are built separately;
there is no universal2 wheel. Linux Python 3.8 uses ABI3 because current official
manylinux/musllinux images no longer include a 3.8 build interpreter. The Linux
release builds opt into `vendored-openssl` and audit/repair their platform tags;
users still need a system CA certificate store for HTTPS. Installing a compatible
wheel does not require Rust, a C compiler or OpenSSL development headers.

An sdist is also provided for source builds. It requires Rust, a C toolchain and
the target platform's TLS build dependencies; optional `vendored-openssl` also
requires Perl and make on Linux. An sdist is not a guarantee of support for
unlisted operating systems or architectures.

## Build and test

From this directory, with ordinary CPython 3.8+ and a Rust toolchain:

```sh
python3 -m venv .venv
source .venv/bin/activate
python -m pip install 'maturin>=1.14,<2'
maturin develop --extras test
python -m pytest
maturin build --release --out dist
```

The default `abi3` Cargo feature targets CPython's Python 3.8 stable ABI. A wheel
tagged `cp38-abi3` can be installed on ordinary CPython 3.8 and newer versions on
the matching platform and architecture. The build interpreter may be newer than
3.8; each supported runtime still needs testing. This does not make one wheel
portable across operating systems, CPU architectures, or glibc/musl variants.

Distribute both ABI3 and version-specific wheels under the same package name and
version. For compatible wheels of the same version, pip prefers the matching
CPython-specific build; if that build is absent, it can select the ABI3 wheel.
This is installation-time selection, not a runtime fallback after an import or
delivery failure. Keep the OS, architecture and platform compatibility in mind.

Build an ABI3 fallback once per platform, then build native wheels with each
selected ordinary CPython interpreter, for example:

```sh
maturin build --release --interpreter python3.12 --out dist-abi3
maturin build --release --no-default-features --interpreter python3.12 --out dist-native-312
maturin build --release --no-default-features --interpreter python3.13 --out dist-native-313
```

`--no-default-features` disables the binding's ABI3 feature and targets the
selected interpreter. Publish the resulting wheels together for one release.
The stable ABI can limit Python-version-specific optimizations in argument
conversion and callback calls. Benchmark both builds for throughput-sensitive
workloads; ABI3 is a compatibility choice, not a promise of identical performance.

The workspace's default members remain the four pure Rust crates;
`cargo check -p aliyun-log-producer-python` or `cargo test --workspace` explicitly
includes this crate and requires Python. Maturin supplies the extension-module
build configuration. PR CI only uploads artifacts; the separate release workflow
publishes on the Python release tags described below.

CI builds one ABI3 wheel per host platform (Ubuntu x86_64, macOS Intel and Windows
x86_64), checks its metadata and stable ABI symbols with `abi3audit --strict`,
and installs that same artifact on CPython 3.8 through 3.14 to run the tests.
Each Python job also builds and tests its version-specific wheel, then exercises
pip's native preference and ABI3 fallback against local wheel directories. Both
builds have the same version, so test installs force replacement to avoid reusing
an already installed build. The final `python-wheels` artifact collects the
3 ABI3 wheels and 21 version-specific wheels on a successful run; after a build
failure it still collects the available wheels. Matrix entries continue after a
sibling fails, and native builds/tests still run after an ABI3 build fails.
This PR workflow does not publish.
Installation requires binary wheels, so a source build cannot mask ABI errors.
These host-platform CI wheels are not a portable manylinux/musllinux release
matrix; Linux release images and native TLS dependency packaging are separate.
Test dependencies are declared in the `test` extra; installers select versions
compatible with the target Python, including older releases for Python 3.8.

From the repository root, validate a wheel directory containing exactly one
artifact (install `abi3audit` in the build environment first):

```sh
python bindings/python/ci/check_wheel.py wheelhouse --audit
# Run on each target interpreter, in a fresh environment:
python bindings/python/ci/check_wheel.py wheelhouse --install
python -m pytest bindings/python/tests -q
# On the interpreter matching the native wheel, in a disposable environment:
python bindings/python/ci/check_wheel.py wheelhouse-native --kind native --install
python -m pytest bindings/python/tests -q
python bindings/python/ci/check_selection.py wheelhouse wheelhouse-native
```

## Release workflow

[Python Release](../../../.github/workflows/python-release.yml) follows a separate
release path from the PR tests. Its matrix is defined in
[ci/release.py](../ci/release.py): currently 35 Linux wheels, 26 desktop wheels and
one sdist. It uses official manylinux/musllinux images and QEMU for other Linux
architectures, and managed Python interpreters for old macOS ARM64 versions.
Matrix entries continue after a sibling fails. The `python-release-builds`
artifact collects the available wheels and sdist even after a build failure.
Release validation and publishing require every configured build to succeed;
extended architectures are not silently omitted from a partial release.

The separate `Python platform build status` workflow updates ABI3 badges after
each completed release-workflow run, including failed and cancelled runs. It
generates the expected ABI3 job names from `ci/release.py` on the default branch,
reads job results through the GitHub API, and writes SVGs plus a `status.json`
run record to the dedicated `python-build-status` branch. Older runs cannot
overwrite newer results. This observer must be on the default branch to run;
badges become available after its first successful update. It needs Actions read
and Contents write access, including permission to update the status branch.

The workflow checks that every artifact has the expected package name and
version, checks the expected artifact count, runs strict ABI3 symbol auditing and
`twine check`, and produces `SHA256SUMS`. Both wheel kinds and the sdist are
attached to the GitHub release. This release workflow does not run the full test
suite on each target; the separate Python CI exercises runtime behavior and pip
selection. The release workflow has not yet been executed or verified end to end.

| Trigger | Result |
| --- | --- |
| Manual dispatch, empty `draft_version` | Build and validate artifacts only; no GitHub release or PyPI upload |
| Manual dispatch, e.g. `draft_version=python-v0.1.0-beta1` | Create a draft GitHub release with all artifacts; no PyPI upload |
| Push `python-v<VERSION>` | Create/update the GitHub release and upload all wheels plus sdist to PyPI |

First update `bindings/python/Cargo.toml` to the intended package version. The tag
must match that version after Python version normalization: for example Cargo
`0.1.0-beta1` corresponds to tag `python-v0.1.0-beta1` and Python version
`0.1.0b1`. There is no automatic version rewrite, and an existing tag must point
to the checkout being built. Prerelease package versions create prerelease GitHub
releases. `python-v*` tags keep Python publishing separate from Rust crate tags.

Configure the repository secret **`PYPI_PASSWORD`** with a PyPI API token permitted
to upload `aliyun-log-producer`. GitHub release creation uses the workflow's
`GITHUB_TOKEN` with `contents: write`; all build jobs have read-only permissions.
Only tag pushes upload to PyPI. `twine --skip-existing` allows retries after a
partial upload but cannot replace an already published file: fixes require a new
version. The release job uploads only `.whl` and `.tar.gz` files to PyPI, not the
checksum file.
