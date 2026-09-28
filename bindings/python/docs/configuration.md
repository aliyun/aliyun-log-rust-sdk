# Python configuration

[简体中文](configuration_cn.md) · [Quick start](quickstart.md) · [Examples](examples.md) · [Error handling](errors.md)

Use the following code to create a Producer and send logs.

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

If creating a Producer or its configuration fails, see [error handling](errors.md).

## Credentials

Producer needs credentials to send logs. Choose one of these options.

### Static credentials

Set the AccessKey ID and AccessKey secret directly, as in the example above.

For STS temporary credentials, also set `security_token`.

These credentials are not refreshed automatically. Use dynamic credentials if you need to refresh temporary credentials.

### Dynamic credentials

Set `credentials_provider` to use dynamic credentials. Producer calls its `get_credentials()` method to obtain credentials.
For temporary credentials, set `expires_at` to the expiration time in Unix seconds so the SDK can refresh them automatically.

## Producer configuration options

| Argument | Type | Default | Meaning and range |
| --- | --- | --- | --- |
| `endpoint` | `str` | Required | SLS endpoint, for example `cn-hangzhou.log.aliyuncs.com`. |
| `user_agent` | `str` | `aliyun-log-python-producer/<version>` | Custom User-Agent sent with HTTP requests to identify your application. |
| `compression` | `str` | `"zstd"` | Log compression format: `"zstd"` or `"lz4"`. |
| `generate_pack_id` | `bool` | `True` | Add PackId for log context queries. Does not deduplicate logs. |
| `batch_size_threshold` | `int` | 1 MiB (`1048576`) | Send a batch when its estimated log size reaches this value. Range: 1–8388608 bytes (8 MiB). |
| `batch_count_threshold` | `int` | `4096` | Send a batch when its log count reaches this value. Range: 1–40960. |
| `linger` | `float` | `2.0` s | Maximum time to wait for more logs before sending. Zero disables the wait. In seconds; range: 0–31536000 (365 days). |
| `buffer_bytes` | `int` | 128 MiB (`134217728`) | Buffer budget for pending logs. When full, new logs are rejected and your application can retry later. This is not a process memory limit. |
| `processing_workers` | `int` | `4` | Number of worker threads processing logs. |
| `callback_capacity` | `int` | `65536` | Maximum logs waiting for callbacks to finish. When full, new sends with callbacks are rejected. |
| `max_attempts` | `int` | `10` | Maximum sends per batch, including the first attempt. Set to 1 to disable retries. |
| `base_backoff` | `float` | `0.2` s | Initial wait before retrying. Must not exceed `max_backoff`. In seconds; must be greater than 0 and at most 31536000 (365 days). |
| `max_backoff` | `float` | `10.0` s | Maximum wait between retries. In seconds; must be greater than 0 and at most 31536000 (365 days). |
| `delivery_timeout` | `float` | `600.0` s | Total time allowed to send a batch, including batching, requests, and retries. In seconds; must be greater than 0 and at most 31536000 (365 days). |
