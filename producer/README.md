# SLS Rust Producer

简体中文 | [English](README_EN.md)

将日志发送到阿里云日志服务（SLS），支持批量发送、失败重试和结果回调。

| 文档                                 | 内容                                     |
| ------------------------------------ | ---------------------------------------- |
| [快速开始](docs/quickstart_cn.md)    | 安装、创建 Producer、发送日志和关闭      |
| [回调](docs/callbacks_cn.md)         | 用最简单的回调查看投递结果               |
| [使用示例](docs/examples_cn.md)      | 回调、flush、关闭、时间戳和多个 Logstore |
| [动态凭证](docs/credentials_cn.md)   | 配置凭证来源，自动缓存和刷新             |
| [配置参考](docs/configuration_cn.md) | 配置项、默认值和取值范围                 |
| [错误处理](docs/errors_cn.md)        | 错误类型、含义和处理建议                 |
| [指标参考](docs/metrics_cn.md)       | 接入统计，查看发送成功、失败和日志大小   |

创建 Producer 后即可发送日志，不需要手动启动。程序运行期间复用 Producer 和 writer，退出前关闭。

日志在后台发送。需要确认是否成功时，请使用回调。日志可能乱序或重复；程序异常退出时，尚未发送的日志可能丢失。

[Python 使用指南](../bindings/python/README.md)
