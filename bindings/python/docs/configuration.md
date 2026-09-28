# Python configuration

[简体中文](configuration_cn.md) · [Quick start](quickstart.md) · [Examples](examples.md)

Pass keyword arguments to `ProducerConfig(...)`. Omitted options use the defaults
below. Configuration objects are immutable; running producers cannot be reconfigured.
Invalid settings raise `ValueError` when constructing the config or producer;
out-of-range integers may raise `OverflowError`.

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

## Connection and credentials

| Argument | Default | Meaning |
| --- | --- | --- |
| `endpoint` | Required | Regional SLS endpoint, e.g. `cn-hangzhou.log.aliyuncs.com` |
| `access_key_id` | `None` | Nonempty AccessKey ID; required together with secret when no provider is supplied |
| `access_key_secret` | `None` | Nonempty AccessKey secret |
| `security_token` | `None` | Optional STS token for fixed temporary credentials |
| `credentials_provider` | `None` | Object with synchronous `get_credentials()` returning `Credentials` |

A provider cannot be combined with fixed key/token arguments. It need not inherit
from a base class. `Credentials` accepts `access_key_id`, `access_key_secret`, optional
`security_token` and optional `expires_at` (absolute Unix seconds as an integer).
Provide the actual expiration for temporary credentials so they can be refreshed;
fixed key/token arguments do not refresh automatically. Construction does not call
the provider. Give credential-fetching operations finite network timeouts.

## Delivery and resource settings

| Argument | Default | Meaning and range |
| --- | --- | --- |
| `user_agent` | `aliyun-log-python-producer/<version>` | HTTP User-Agent header; replaces the default. Must be a valid HTTP header value. |
| `compression` | `"zstd"` | Use `"zstd"` or `"lz4"`. |
| `generate_pack_id` | `True` | Add PackId for SLS log context queries. Does not deduplicate logs. |
| `batch_size_threshold` | 1 MiB (`1048576`) | Estimated log bytes that trigger a batch send. Range: 1–8388608 bytes (8 MiB). |
| `batch_count_threshold` | `4096` | Log count that triggers a batch send. Range: 1–40960. |
| `linger` | `2.0` s | Maximum wait to collect more logs, starting with the oldest log in a batch. Zero disables this wait. |
| `buffer_bytes` | 128 MiB (`134217728`) | Soft budget for estimated bytes of accepted logs still awaiting delivery. Positive integer; not a process memory limit. |
| `processing_workers` | `4` | Number of workers preparing logs for delivery. Positive integer. |
| `callback_capacity` | `65536` | Maximum accepted logs whose callbacks have not finished, including those still in delivery. Positive integer; sends without callbacks do not consume this capacity. |
| `max_attempts` | `10` | Maximum delivery attempts including the first request. Positive integer; 1 disables retries. |
| `base_backoff` | `0.2` s | Initial retry backoff. Must be positive and no greater than `max_backoff`. |
| `max_backoff` | `10.0` s | Maximum retry backoff; actual delays are randomized. Must be positive. |
| `delivery_timeout` | `600.0` s | Total delivery deadline, including batching, requests and retries. Must be positive. |

Time arguments are finite numbers in seconds (integers or floats). Linger may be zero; the other durations must be positive. All are limited to 31536000 seconds (365 days).

A batch is sent when its log count, estimated bytes or linger condition is met.
The byte threshold excludes group metadata such as source and topic; the last log
may take a batch beyond the threshold. Linger only limits batching delay, not delivery latency.
The delivery deadline starts at the oldest accepted log in a batch and is shared by
that batch; it is not a wait timeout for flush or close.
