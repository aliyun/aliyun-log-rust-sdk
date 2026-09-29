# Rust 错误处理

[English](errors.md) · [使用示例](examples_cn.md) · [配置参考](configuration_cn.md)

调用方法时返回的错误见 `ProducerError`。日志被接收后，如果发送失败，会通过回调收到 `DeliveryError`。
`flush` 和 `close` 不会报告单条日志的发送失败。

## 调用方法时的错误

| `ProducerError` | 含义与处理 |
| --- | --- |
| `Config` | 配置不正确。根据错误信息检查配置项。 |
| `Creation` | Producer 启动失败。根据错误信息检查运行环境。 |
| `EnqueueFull` | 队列已满，未接收这条日志。可以稍后重试。 |
| `Closed` | Producer 正在关闭或已关闭。请停止发送，或创建新的 Producer。 |
| `InvalidInput` | Project 或 Logstore 名称不正确。检查传入的名称。 |
| `Reentrant` | 在回调中等待同一个 Producer。请在回调外调用 flush 或 close。 |
| `Internal` | Producer 运行出错。保留错误信息以便排查。 |

`EnqueueFull` 和 `Closed` 不会触发回调。可以用 `into_log()` 取回未发送的日志，稍后重试或另行保存。
重试示例见[使用示例](examples_cn.md)。

## 回调中的发送错误

| `DeliveryError` | 含义与处理 |
| --- | --- |
| `Server` | SLS 返回错误。根据错误码检查权限、目标 Logstore 等设置。 |
| `Network` | 网络请求失败。检查网络连接和 SLS 服务地址。 |
| `Credentials` | 获取凭证失败。检查凭证来源是否可用。 |
| `Timeout` | 处理或开始发送前发现已超过 `delivery_timeout` 软时限。检查网络和服务状态，必要时调整超时时间。 |
| `InvalidResponse` | 无法识别服务返回的内容。检查服务地址和代理设置。 |
| `Internal` | 日志处理失败。保留错误信息以便排查。 |

通过 `error_code()`、`http_status()` 和 `request_id()` 查看 SLS 返回的错误详情。
排查服务错误时，建议保留错误信息和 request ID。权限或身份验证失败属于 `Server`。

## 凭证错误

创建凭证或编写动态凭证获取逻辑时，可能遇到 `CredentialsError`：

| 错误 | 含义 |
| --- | --- |
| `InvalidAccessKey` | AccessKey ID 或 Secret 为空。 |
| `Expired` | 凭证已过期。 |
| `Timeout` | 获取凭证超时。 |
| `Throttled` | 暂时无法再次获取凭证，请稍后重试。 |
| `Provider` | 凭证来源出错，具体原因见错误信息。 |
