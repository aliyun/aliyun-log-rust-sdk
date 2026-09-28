# Python 配置参考

[English](configuration.md) · [快速开始](quickstart_cn.md) · [使用示例](examples_cn.md)

可以参照下面代码创建 Producer，并写入日志。

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

创建 Producer 或配置时可能抛出 `ValueError` 或 `OverflowError`。

## 配置凭证

Producer 需要配置凭证才能写入日志，有两种方式可以配置凭证，选择其中一种即可。

### 静态凭证

直接设置 AccessKey ID 和 AccessKey Secret，如上面的示例。

| 参数 | 说明 |
| --- | --- |
| `access_key_id` | AccessKey ID，不能为空。 |
| `access_key_secret` | AccessKey Secret，不能为空。 |
| `security_token` | 使用 STS 临时凭证时填写 Token，否则无需设置。 |

这种方式不会自动更新凭证。需要自动刷新临时凭证时，请使用动态凭证。

### 动态凭证

通过 `credentials_provider` 提供一个带有 `get_credentials()` 方法的对象。
Producer 会调用这个方法获取凭证；方法需要同步返回 `Credentials`。

`Credentials` 包含 `access_key_id`、`access_key_secret`，也可以设置 `security_token`。
使用临时凭证时，请通过 `expires_at` 提供到期时间（整数形式的 Unix 秒），供 SDK 自动刷新凭证。

## Producer 配置选项

| 参数 | 默认值 | 说明与范围 |
| --- | --- | --- |
| `endpoint` | 必填 | SLS 服务地址，例如 `cn-hangzhou.log.aliyuncs.com`。 |
| `user_agent` | `aliyun-log-python-producer/<version>` | HTTP 请求中的 User-Agent。需要自定义时设置，必须是合法的 HTTP 请求头值。 |
| `compression` | `"zstd"` | 日志压缩方式，可选 `"zstd"` 或 `"lz4"`。 |
| `generate_pack_id` | `True` | 添加 PackId，便于查询日志上下文；不能用于日志去重。 |
| `batch_size_threshold` | 1 MiB (`1048576`) | 一批日志的预估大小达到此值时发送。范围 1–8388608 字节（8 MiB）。 |
| `batch_count_threshold` | `4096` | 一批日志的条数达到此值时发送。范围 1–40960。 |
| `linger` | `2.0` s | 等待更多日志的最长时间，到时发送；0 表示不等待。以秒为单位，支持整数或小数，范围 0–31536000 秒（365 天）。 |
| `buffer_bytes` | 128 MiB (`134217728`) | 等待投递完成的日志最多可占用多少预估字节。必须是正整数，实际可能略超此值，不代表进程内存上限。 |
| `processing_workers` | `4` | 处理日志的工作线程数，必须是正整数。 |
| `callback_capacity` | `65536` | 最多允许多少条日志等待回调完成，必须是正整数。未设置回调的日志不计入。 |
| `max_attempts` | `10` | 每批日志最多发送多少次，包含首次发送。必须是正整数；1 表示不重试。 |
| `base_backoff` | `0.2` s | 首次重试前的等待时间，不能超过 `max_backoff`。以秒为单位，支持整数或小数，必须大于 0，不超过 31536000 秒（365 天）。 |
| `max_backoff` | `10.0` s | 重试之间的最长等待时间，实际等待时间会随机调整。以秒为单位，支持整数或小数，必须大于 0，不超过 31536000 秒（365 天）。 |
| `delivery_timeout` | `600.0` s | 每批日志允许的总发送时间，包含等待合批、请求和重试。以秒为单位，支持整数或小数，必须大于 0，不超过 31536000 秒（365 天）。 |
