# SLS Rust Producer

[English](README.md)

将日志发送到阿里云日志服务（SLS），支持自动合批、重试和可选的投递回调。

| 文档 | 内容 |
| --- | --- |
| [快速开始](docs/quickstart_cn.md) | 安装、创建 Producer、发送日志和关闭 |
| [使用示例](docs/examples_cn.md) | 回调、flush、关闭、时间戳和多个 Logstore |
| [配置参考](docs/configuration_cn.md) | 配置项、默认值和取值范围 |
| [指标参考](docs/metrics.md) | 指标名称、单位和记录语义 |

为 endpoint 和凭证创建 Producer，再为各个 Project / Logstore 获取 writer。
创建后即已启动，不需要额外调用 `start()`。后续发送应复用 Producer 和 writer。

`send` 成功仅表示日志已在本地接收，最终投递结果通过 callback 获取。
退出前关闭 Producer，等待已接收日志和回调完成。投递不保证顺序，重试可能产生重复日志；
待发送日志不会持久化到磁盘。

[Python 使用指南](../bindings/python/README_CN.md)
