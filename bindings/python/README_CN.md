# SLS Python Producer

[English](README.md)

将日志发送到阿里云日志服务（SLS），支持批量发送、失败重试和结果回调。

支持 Python 3.8 及以上版本，支持 macOS、Windows、Linux，以及 x86_64、ARM64 等架构。详细信息请参阅[平台支持](docs/platforms_cn.md)。

| 文档 | 内容 |
| --- | --- |
| [快速开始](docs/quickstart_cn.md) | 安装、创建 Producer、发送日志和关闭 |
| [使用示例](docs/examples_cn.md) | 回调、flush、关闭、时间戳和多个 Logstore |
| [配置参考](docs/configuration_cn.md) | 配置项、默认值和取值范围 |
| [错误处理](docs/errors_cn.md) | 错误类型、含义和处理建议 |

创建 Producer 后即可发送日志，不需要手动启动。程序运行期间复用 Producer 和 writer，退出前关闭。

日志在后台发送。需要确认是否成功时，请使用回调。日志可能乱序或重复；程序异常退出时，尚未发送的日志可能丢失。

[Rust 使用指南](../../producer/README_CN.md)
