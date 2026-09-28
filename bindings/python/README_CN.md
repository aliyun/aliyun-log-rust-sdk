# SLS Python Producer

[English](README.md)

将日志发送到阿里云日志服务（SLS），支持自动合批、重试和可选的投递回调。

| 文档 | 内容 |
| --- | --- |
| [快速开始](docs/quickstart_cn.md) | 安装、创建 Producer、发送日志和关闭 |
| [使用示例](docs/examples_cn.md) | 回调、flush、关闭、时间戳和多个 Logstore |
| [配置参考](docs/configuration_cn.md) | 配置项、默认值和取值范围 |

为 endpoint 和凭证创建 Producer，再为各个 Project / Logstore 获取 writer。
创建后即已启动，不需要额外调用 `start()`。后续发送应复用 Producer 和 writer。

`send` 成功仅表示日志已在本地接收，最终投递结果通过 callback 获取。
退出前关闭 Producer，等待已接收日志和回调完成。投递不保证顺序，重试可能产生重复日志；
待发送日志不会持久化到磁盘。

## 主要平台

| 平台 | x86_64 | ARM64 |
| --- | --- | --- |
| Linux（glibc） | [![manylinux2014-x86_64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-x86_64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux2014-aarch64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-aarch64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux（musl / Alpine） | [![musllinux_1_2-x86_64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-x86_64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![musllinux_1_2-aarch64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-aarch64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| macOS | [![x86_64-apple-darwin ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/x86_64-apple-darwin.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![aarch64-apple-darwin ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/aarch64-apple-darwin.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Windows | [![x86_64-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/x86_64-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![aarch64-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/aarch64-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |

## 扩展架构

| 平台 | x86 / i686 | ARMv7 | ppc64le | s390x | RISC-V 64 |
| --- | --- | --- | --- | --- | --- |
| Linux（glibc） | [![manylinux2014-i686 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-i686.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux_2_31-armv7l ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux_2_31-armv7l.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux2014-ppc64le ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-ppc64le.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux2014-s390x ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux2014-s390x.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![manylinux_2_39-riscv64 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/manylinux_2_39-riscv64.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) |
| Linux（musl / Alpine） | [![musllinux_1_2-i686 ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-i686.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | [![musllinux_1_2-armv7l ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/musllinux_1_2-armv7l.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | — | — | — |
| Windows | [![i686-pc-windows-msvc ABI3](https://raw.githubusercontent.com/aliyun/aliyun-log-rust-sdk/python-build-status/i686-pc-windows-msvc.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/python-release.yml) | — | — | — | — |

[Rust 使用指南](../../producer/README_CN.md)
