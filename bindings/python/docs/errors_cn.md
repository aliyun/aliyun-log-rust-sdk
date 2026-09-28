# Python 错误处理

[English](errors.md) · [使用示例](examples_cn.md) · [配置参考](configuration_cn.md)

调用方法时的错误通过异常报告。日志被接收后，如果发送失败，会通过 `on_delivery` 收到 `DeliveryError`。
`flush()` 和 `close()` 不会报告单条日志的发送失败。

## 调用方法时的异常

| 异常 | 含义与处理 |
| --- | --- |
| `EnqueueFullError` | 队列已满，未接收这条日志。可以稍后重试。 |
| `ProducerClosedError` | Producer 正在关闭或已关闭。请停止发送，或创建新的 Producer。 |
| `ConfigError` | 配置错误，包括缺失或未知参数、类型、范围和配置冲突。 |
| `InvalidArgumentError` | 日志、凭据快照、目标名称或其他方法参数不正确。 |
| `ProducerError` | Producer 启动或运行出错，也可能是调用方式不正确。根据错误信息排查。 |

以上异常均继承 `ProducerError`。使用 `except ProducerError` 可统一捕获 SDK 的同步调用失败，
也可以按需捕获具体子类。发送被拒绝时不会触发投递回调。重试示例见[使用示例](examples_cn.md)。

## 回调中的发送错误

`on_delivery` 在成功时收到 `None`，失败时收到 `DeliveryError`。`DeliveryError` 不会作为异常抛出，请在回调中处理。

| `error.kind` | 含义与处理 |
| --- | --- |
| `server` | SLS 返回错误。根据错误码检查权限、目标 Logstore 等设置。 |
| `network` | 网络请求失败。检查网络连接和 SLS 服务地址。 |
| `credentials` | 获取凭证失败。检查凭证来源是否可用。 |
| `timeout` | 处理或开始发送前发现已超过 `delivery_timeout` 软时限。检查网络和服务状态，必要时调整超时时间。 |
| `invalid_response` | 无法识别服务返回的内容。检查服务地址和代理设置。 |
| `internal` | 日志处理失败。保留错误信息以便排查。 |
| `unknown` | 未归类的发送错误，具体原因见 `message`。 |

`message` 是错误说明；SLS 返回的错误还可通过 `error_code`、`http_status` 和 `request_id` 查看详情。
排查服务错误时，建议保留错误信息和 request ID。权限或身份验证失败属于 `server`。

创建 Producer 时首次获取凭据失败会抛出 `ProducerError`。后台刷新失败保留缓存凭据并重试，
不会直接通过 `on_delivery` 通知。应用 callback 自身抛出的异常保留原类型，由
`sys.unraisablehook` 报告。
