# Python 配置参考

可以参照下面代码创建 Producer，并写入日志。

```python
import os
from aliyun_log_producer import Producer, ProducerConfig

config = ProducerConfig(
    endpoint="cn-hangzhou.log.aliyuncs.com",
    access_key_id=os.environ["ALIBABA_CLOUD_ACCESS_KEY_ID"],
    access_key_secret=os.environ["ALIBABA_CLOUD_ACCESS_KEY_SECRET"],
)
with Producer(config) as producer:
    producer.writer("my-project", "my-logstore").send({"message": "hello"})
```

创建 Producer 或配置失败时，请参照[错误处理](errors_cn.md)。

## 配置凭证

Producer 需要凭证才能写入日志。可以选择以下两种方式。

### 静态凭证

直接设置 AccessKey ID 和 AccessKey Secret：

```python
config = ProducerConfig(
    endpoint="cn-hangzhou.log.aliyuncs.com",
    access_key_id=os.environ["ALIBABA_CLOUD_ACCESS_KEY_ID"],
    access_key_secret=os.environ["ALIBABA_CLOUD_ACCESS_KEY_SECRET"],
)
```

使用 STS 临时凭证时，还需填写 `security_token`。

这种方式不会自动更新凭证。需要自动刷新临时凭证时，请使用动态凭证。

### 动态凭证

通过 `credentials_provider` 配置动态凭证。Producer 会调用它的 `get_credentials()` 方法获取凭证。
使用临时凭证时，请设置 `expires_at`（到期时间的 Unix 秒），供 SDK 自动刷新凭证。

创建 `Producer` 时会尝试获取一次凭证，失败时抛出 `ProducerError`。
SDK 自动缓存凭证，在 `expires_at` 到期前提前刷新，并在刷新失败时保留旧凭证、
自动重试。未设置 `expires_at` 时只缓存，不自动刷新。

provider 中的 I/O 必须设置有限超时：阻塞刷新会延迟投递回调和 `close()`。
多个 Producer 共用的 provider 应支持并发调用。

## Producer 配置选项

| 参数                    | 类型    | 默认值                                 | 说明与范围                                                                                                                                                                                                  |
| ----------------------- | ------- | -------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `endpoint`              | `str`   | 必填                                   | SLS 服务地址，例如 `cn-hangzhou.log.aliyuncs.com`。                                                                                                                                                         |
| `user_agent`            | `str`   | `aliyun-log-python-producer/<version>` | 自定义 HTTP 请求中的 User-Agent，用于标识应用。                                                                                                                                                             |
| `compression`           | `str`   | `"zstd"`                               | 日志压缩方式，可选 `"zstd"` 或 `"lz4"`。                                                                                                                                                                    |
| `generate_pack_id`      | `bool`  | `True`                                 | 自动添加 PackId，写入日志库后可进行上下文查询；不能用于日志去重。                                                                                                                                           |
| `batch_size_threshold`  | `int`   | 1 MiB (`1048576`)                      | 多次调用 `send` 会进行攒批，每批日志的预估大小达到此值时发送。范围 1–8388608 字节（8 MiB），不是单条日志大小上限。                                                                                          |
| `batch_count_threshold` | `int`   | `4096`                                 | 多次调用 `send` 会进行攒批，每批日志达到此条数时发送。范围 1–40960。                                                                                                                                        |
| `linger`                | `float` | `2.0` s                                | 日志被缓存以进行攒批的最长时间，到时立即进入发送流程。单位为秒，范围 0.01–31536000 秒（365 天）。                                                                                                           |
| `buffer_bytes`          | `int`   | 128 MiB (`134217728`)                  | 待发送日志的预估原始数据量预算。达到后，新日志会被拒绝；需要由应用稍后重试。这不是进程内存上限。                                                                                                            |
| `processing_workers`    | `int`   | `2`                                    | 处理日志的工作线程数，至少为 1。                                                                                                                                                                            |
| `callback_capacity`     | `int`   | `65536`                                | 尚未完成回调的日志数量上限。不带回调的发送不占用此容量。达到后，带回调的新日志会被拒绝。范围 1024–1048576（`1024 * 1024`）。                                                                                |
| `max_attempts`          | `int`   | `10`                                   | 每批日志最大发送次数，包含首次发送。至少为 1；1 表示不重试。                                                                                                                                                |
| `base_backoff`          | `float` | `0.2` s                                | 重试等待时间的初始上限。单位为秒，范围 0.1–60 秒，且不能超过 `max_backoff`。                                                                                                                                |
| `max_backoff`           | `float` | `10.0` s                               | 重试等待时间的最大上限。单位为秒，范围 0.1–600 秒。                                                                                                                                                         |
| `delivery_timeout`      | `float` | `600.0` s                              | 从批次停止聚合（封批）开始计算的投递软时限，包含排队、处理、请求和重试，不包含聚合等待（`linger`）和回调。处理或发送前检查；正在进行的请求使用自身超时，允许稍晚完成。单位为秒，范围 60–604800 秒（7 天）。 |

以上范围均包含边界值。每批日志达到大小、条数或等待时间中的任一条件后进入发送流程。

## 回调与终结回调的限制

投递回调中可以调用 `writer.send()`。不支持在投递回调或 `get_credentials()`
中调用同一 Producer 的 `flush()` 或 `close()`：这些操作可能等待当前正在执行
自身的线程，导致死锁。请在回调返回后，由应用代码执行刷新和关闭。

不支持在用户定义的 `__del__` 或终结回调（包括 `weakref.finalize` 回调）中调用
`writer.send()`、`Producer.flush()` 或 `Producer.close()`。这一限制也适用于
回调对象：SDK 释放其引用时可能触发对象终结。请使用 `with Producer(...)`，
或在应用正常退出时显式调用 `close()`，不要依赖终结回调保证日志投递。
这些不支持的调用不保证执行结果，也不保证抛出特定异常。

自定义 provider 的简短示例见[动态凭证](credentials_cn.md)。

## 相关文档

- [快速开始](quickstart_cn.md)
- [使用示例](examples_cn.md)
- [错误处理](errors_cn.md)
