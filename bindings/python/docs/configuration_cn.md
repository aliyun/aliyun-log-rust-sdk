# Python 配置参考

[English](configuration.md) · [快速开始](quickstart_cn.md) · [使用示例](examples_cn.md)

通过 `ProducerConfig(...)` 的关键字参数配置，未传入的选项使用下表默认值。
配置对象不可变，创建 Producer 后不能修改其配置。
配置错误在构造配置或 Producer 时抛出 `ValueError`；整数超出类型范围时可能抛出 `OverflowError`。

```python
import os
from aliyun_log_producer import Producer, ProducerConfig

config = ProducerConfig(
    endpoint="cn-hangzhou.log.aliyuncs.com",
    access_key_id=os.environ["ALIBABA_CLOUD_ACCESS_KEY_ID"],
    access_key_secret=os.environ["ALIBABA_CLOUD_ACCESS_KEY_SECRET"],
    linger=0.1,
    delivery_timeout=60,
)
with Producer(config) as producer:
    producer.writer("my-project", "my-logstore").send({"message": "hello"})
```

## 连接与凭证

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `endpoint` | 必填 | SLS 地域 endpoint，例如 `cn-hangzhou.log.aliyuncs.com` |
| `access_key_id` | `None` | 非空的 AccessKey ID；未使用 provider 时，必须与 Secret 一起提供 |
| `access_key_secret` | `None` | 非空的 AccessKey Secret |
| `security_token` | `None` | 固定临时凭证的可选 STS Token |
| `credentials_provider` | `None` | 提供同步 `get_credentials()` 方法并返回 `Credentials` 的对象 |

provider 不能与固定密钥或 Token 同时设置，也不要求继承特定基类。
`Credentials` 接收 `access_key_id`、`access_key_secret`，以及可选的 `security_token` 和
`expires_at`（整数形式的绝对 Unix 秒）。临时凭证应携带真实到期时间，以便刷新；
直接配置固定密钥和 Token 不会自动刷新。构造时不会调用 provider；获取凭证涉及网络时应设置有限超时。

## 发送与资源配置

| 参数 | 默认值 | 说明与范围 |
| --- | --- | --- |
| `user_agent` | `aliyun-log-python-producer/<version>` | HTTP User-Agent 请求头；覆盖默认值，必须是合法的 HTTP 请求头值。 |
| `compression` | `"zstd"` | 使用 `"zstd"` 或 `"lz4"`。 |
| `generate_pack_id` | `True` | 添加 PackId，便于 SLS 日志上下文查询；不提供去重。 |
| `batch_size_threshold` | 1 MiB (`1048576`) | 触发合批发送的预估日志字节数，范围 1–8388608 字节（8 MiB）。 |
| `batch_count_threshold` | `4096` | 触发合批发送的日志条数，范围 1–40960。 |
| `linger` | `2.0` s | 从一批中最早接收的日志开始，等待更多日志的最长时间；0 表示不等待。 |
| `buffer_bytes` | 128 MiB (`134217728`) | 已接收但尚未完成投递日志的预估字节软限制。正整数，不是进程内存上限。 |
| `processing_workers` | `4` | 用于准备待投递日志的工作线程数，正整数。 |
| `callback_capacity` | `65536` | 已接收但回调尚未执行完的日志数量上限，含尚在投递中的日志。正整数；无回调的发送不占此额度。 |
| `max_attempts` | `10` | 最多投递尝试次数，含首次请求。正整数；1 表示不重试。 |
| `base_backoff` | `0.2` s | 初始重试退避时间，必须大于 0 且不超过 `max_backoff`。 |
| `max_backoff` | `10.0` s | 重试退避上限，实际等待带随机性。必须大于 0。 |
| `delivery_timeout` | `600.0` s | 总投递期限，包含攒批、请求和重试等等待。必须大于 0。 |

时间参数以秒为单位，可用整数或浮点数，必须是有限数值。`linger` 可为 0，其余时间参数必须大于 0，均不得超过 31536000 秒（365 天）。

按条数、预估字节数或 `linger` 任一条件满足时触发发送。字节阈值不包含 source、topic 等批次元数据；
最后加入的日志可能使一批略超阈值。`linger` 仅约束攒批等待，不保证日志在该时间内送达。
`delivery_timeout` 从同一批最早接收的日志起计算，同批日志共用该期限；它不是 flush / close 的等待超时。
