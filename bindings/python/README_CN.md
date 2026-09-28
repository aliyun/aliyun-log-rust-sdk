# SLS Python Producer

[English](README.md)

将日志发送到阿里云日志服务（SLS），支持批量发送、失败重试和结果回调。

| 文档 | 内容 |
| --- | --- |
| [快速开始](docs/quickstart_cn.md) | 安装、创建 Producer、发送日志和关闭 |
| [使用示例](docs/examples_cn.md) | 回调、flush、关闭、时间戳和多个 Logstore |
| [配置参考](docs/configuration_cn.md) | 配置项、默认值和取值范围 |
| [错误处理](docs/errors_cn.md) | 错误类型、含义和处理建议 |

创建 Producer 后即可发送日志，不需要手动启动。程序运行期间复用 Producer 和 writer，退出前关闭。

日志在后台发送。需要确认是否成功时，请使用回调。日志可能乱序或重复；程序异常退出时，尚未发送的日志可能丢失。

## 提供 Python 版本专用 wheel 的平台

提供 Python 3.10–3.14 的专用 wheel，同时提供 ABI3 wheel，兼容 Python 3.8 及以上版本。

| 平台 | 架构 | ABI3 构建状态 |
| --- | --- | --- |
| Windows | x86_64 | [![x86_64-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/x86_64-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| macOS | ARM64 | [![aarch64-apple-darwin ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/aarch64-apple-darwin.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (glibc) | x86_64 | [![manylinux2014-x86_64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-x86_64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (glibc) | ARM64 | [![manylinux2014-aarch64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-aarch64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (musl / Alpine) | x86_64 | [![musllinux_1_2-x86_64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-x86_64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (musl / Alpine) | ARM64 | [![musllinux_1_2-aarch64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-aarch64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |

## 仅提供 ABI3 wheel 的平台

仅提供 ABI3 wheel，兼容 Python 3.8 及以上版本。不支持 Python 3.6/3.7。

| 平台 | 架构 | ABI3 构建状态 |
| --- | --- | --- |
| macOS | x86_64 | [![x86_64-apple-darwin ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/x86_64-apple-darwin.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Windows | x86 / i686 | [![i686-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/i686-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Windows | ARM64 | [![aarch64-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/aarch64-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (glibc) | i686 | [![manylinux2014-i686 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-i686.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (glibc) | ARMv7 | [![manylinux_2_31-armv7l ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux_2_31-armv7l.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (glibc) | ppc64le | [![manylinux2014-ppc64le ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-ppc64le.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (glibc) | s390x | [![manylinux2014-s390x ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-s390x.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (glibc) | RISC-V 64 | [![manylinux_2_39-riscv64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux_2_39-riscv64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (musl / Alpine) | i686 | [![musllinux_1_2-i686 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-i686.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux (musl / Alpine) | ARMv7 | [![musllinux_1_2-armv7l ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-armv7l.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |

[Rust 使用指南](../../producer/README_CN.md)
