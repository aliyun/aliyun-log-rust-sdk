# SLS Producer for Python

[简体中文](README_CN.md)

Send logs to Alibaba Cloud Simple Log Service (SLS), with automatic batching,
retries, and callbacks to check delivery results.

| Guide | Contents |
| --- | --- |
| [Quick start](docs/quickstart.md) | Install, create a producer, send a log and close |
| [Usage examples](docs/examples.md) | Callbacks, flush, shutdown, timestamps and multiple logstores |
| [Configuration](docs/configuration.md) | Options, defaults and valid values |

The Producer starts when you create it. Reuse it and its writers while your application runs, and close it before exiting.

Logs are sent in the background. Use a callback to check whether delivery succeeded. Logs may arrive out of order or more than once. Pending logs may be lost if the application crashes.

## Main platforms

| Platform | x86_64 | ARM64 |
| --- | --- | --- |
| Linux (glibc) | [![manylinux2014-x86_64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-x86_64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux2014-aarch64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-aarch64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (musl / Alpine) | [![musllinux_1_2-x86_64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-x86_64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![musllinux_1_2-aarch64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-aarch64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| macOS | [![x86_64-apple-darwin ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/x86_64-apple-darwin.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![aarch64-apple-darwin ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/aarch64-apple-darwin.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Windows | [![x86_64-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/x86_64-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![aarch64-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/aarch64-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |

## Extended architectures

| Platform | x86 / i686 | ARMv7 | ppc64le | s390x | RISC-V 64 |
| --- | --- | --- | --- | --- | --- |
| Linux (glibc) | [![manylinux2014-i686 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-i686.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux_2_31-armv7l ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux_2_31-armv7l.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux2014-ppc64le ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-ppc64le.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux2014-s390x ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-s390x.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux_2_39-riscv64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux_2_39-riscv64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (musl / Alpine) | [![musllinux_1_2-i686 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-i686.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![musllinux_1_2-armv7l ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-armv7l.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | — | — | — |
| Windows | [![i686-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/i686-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | — | — | — | — |

[Rust guide](../../producer/README.md)
