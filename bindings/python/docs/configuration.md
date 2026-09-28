# Python configuration

[简体中文](configuration_cn.md) · [Quick start](quickstart.md) · [Examples](examples.md)

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

Creating a Producer or its configuration may raise `ValueError` or `OverflowError`.

## Credentials

Producer needs credentials to send logs. Choose one of the following two options.

### Static credentials

Set the AccessKey ID and AccessKey secret directly, as in the example above.

| Argument | Description |
| --- | --- |
| `access_key_id` | AccessKey ID. Must not be empty. |
| `access_key_secret` | AccessKey secret. Must not be empty. |
| `security_token` | STS token when using temporary credentials. Otherwise, omit it. |

These credentials are not refreshed automatically. Use dynamic credentials if you need to refresh temporary credentials.

### Dynamic credentials

Set `credentials_provider` to an object with a `get_credentials()` method.
Producer calls this method to obtain credentials. The method must return `Credentials` synchronously.

`Credentials` takes `access_key_id`, `access_key_secret`, and an optional `security_token`.
For temporary credentials, set `expires_at` to the expiration time (integer Unix seconds) so the SDK can refresh them automatically.

## Producer configuration options

| Argument | Default | Meaning and range |
| --- | --- | --- |
| `endpoint` | Required | SLS endpoint, for example `cn-hangzhou.log.aliyuncs.com`. |
| `user_agent` | `aliyun-log-python-producer/<version>` | User-Agent sent with HTTP requests. Set it to override the default; must be a valid HTTP header value. |
| `compression` | `"zstd"` | Log compression format: `"zstd"` or `"lz4"`. |
| `generate_pack_id` | `True` | Add PackId for log context queries. Does not deduplicate logs. |
| `batch_size_threshold` | 1 MiB (`1048576`) | Send a batch when its estimated log size reaches this value. Range: 1–8388608 bytes (8 MiB). |
| `batch_count_threshold` | `4096` | Send a batch when its log count reaches this value. Range: 1–40960. |
| `linger` | `2.0` s | Maximum time to wait for more logs before sending. Zero disables the wait. Accepts integer or fractional seconds; range: 0–31536000 (365 days). |
| `buffer_bytes` | 128 MiB (`134217728`) | Budget for estimated bytes of logs awaiting delivery. Must be a positive integer; usage may slightly exceed it. This is not a process memory limit. |
| `processing_workers` | `4` | Number of worker threads processing logs. Must be a positive integer. |
| `callback_capacity` | `65536` | Maximum number of logs waiting for their callbacks to finish. Must be a positive integer. Logs without callbacks do not count. |
| `max_attempts` | `10` | Maximum sends per batch, including the first attempt. Must be a positive integer; 1 disables retries. |
| `base_backoff` | `0.2` s | Initial wait before retrying. Must not exceed `max_backoff`. Accepts integer or fractional seconds; must be greater than 0 and at most 31536000 (365 days). |
| `max_backoff` | `10.0` s | Maximum wait between retries. Actual waits are randomized. Accepts integer or fractional seconds; must be greater than 0 and at most 31536000 (365 days). |
| `delivery_timeout` | `600.0` s | Total time allowed to send a batch, including batching, requests, and retries. Accepts integer or fractional seconds; must be greater than 0 and at most 31536000 (365 days). |
