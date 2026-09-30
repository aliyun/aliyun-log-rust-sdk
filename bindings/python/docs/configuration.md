# Python configuration

Use the following code to create a Producer and send logs.

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

If creating a Producer or its configuration fails, see [error handling](errors.md).

## Credentials

Producer needs credentials to send logs. Choose one of these options.

### Static credentials

Set the AccessKey ID and AccessKey secret directly:

```python
config = ProducerConfig(
    endpoint="cn-hangzhou.log.aliyuncs.com",
    access_key_id=os.environ["ALIBABA_CLOUD_ACCESS_KEY_ID"],
    access_key_secret=os.environ["ALIBABA_CLOUD_ACCESS_KEY_SECRET"],
)
```

For STS temporary credentials, also set `security_token`.

These credentials are not refreshed automatically. Use dynamic credentials if you need to refresh temporary credentials.

### Dynamic credentials

Set `credentials_provider` to use dynamic credentials. Producer calls its `get_credentials()` method to obtain credentials.
For temporary credentials, set `expires_at` to the expiration time in Unix seconds so the SDK can refresh them automatically.

Creating a `Producer` attempts to fetch credentials once; failure raises
`ProducerError`. The SDK caches credentials, automatically refreshes them before
`expires_at`, and retries failed refreshes while retaining the previous credentials.
Without `expires_at`, credentials are cached without automatic refresh.

Provider I/O must have finite timeouts: a blocked refresh delays delivery callbacks
and `close()`. Providers shared by multiple producers must support concurrent calls.

## Callback and finalizer restrictions

Delivery callbacks may call `writer.send()`. Calling the same producer's `flush()`
or `close()` from its delivery callback or `get_credentials()` is unsupported:
these calls can wait on the thread currently executing them and deadlock. Perform
flush and shutdown from application code after returning from the callback.

Calling `writer.send()`, `Producer.flush()` or `Producer.close()` from user-defined
`__del__` methods or finalizers (including `weakref.finalize` callbacks) is
unsupported. This also applies to callback objects: releasing the SDK's reference
can trigger their finalization. Use `with Producer(...)` or explicitly call
`close()` during normal application shutdown; do not rely on finalizers for delivery.
Unsupported calls have no guaranteed outcome or specific exception.

## Producer configuration options

| Argument                | Type    | Default                                | Meaning and range                                                                                                                                                                                                                                                                                                               |
| ----------------------- | ------- | -------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `endpoint`              | `str`   | Required                               | SLS endpoint, for example `cn-hangzhou.log.aliyuncs.com`.                                                                                                                                                                                                                                                                       |
| `user_agent`            | `str`   | `aliyun-log-python-producer/<version>` | Custom User-Agent sent with HTTP requests to identify your application.                                                                                                                                                                                                                                                         |
| `compression`           | `str`   | `"zstd"`                               | Log compression format: `"zstd"` or `"lz4"`.                                                                                                                                                                                                                                                                                    |
| `generate_pack_id`      | `bool`  | `True`                                 | Automatically add PackId to enable context queries after logs are written to the Logstore. Does not deduplicate logs.                                                                                                                                                                                                           |
| `batch_size_threshold`  | `int`   | 1 MiB (`1048576`)                      | Accumulate logs across multiple `send` calls and send a batch when its estimated log size reaches this value. Range: 1–8388608 bytes (8 MiB); not a single-log size limit.                                                                                                                                                      |
| `batch_count_threshold` | `int`   | `4096`                                 | Accumulate logs across multiple `send` calls and send a batch when its log count reaches this value. Range: 1–40960.                                                                                                                                                                                                            |
| `linger`                | `float` | `2.0` s                                | Maximum time logs are cached for accumulation; when elapsed, the batch immediately enters the sending flow. In seconds; range: 0.01–31536000 (365 days).                                                                                                                                                                        |
| `buffer_bytes`          | `int`   | 128 MiB (`134217728`)                  | Estimated raw-data budget for pending logs. When full, new logs are rejected and your application can retry later. This is not a process memory limit.                                                                                                                                                                          |
| `processing_workers`    | `int`   | `2`                                    | Number of worker threads processing logs. Must be at least 1.                                                                                                                                                                                                                                                                   |
| `callback_capacity`     | `int`   | `65536`                                | Maximum number of logs whose callbacks have not yet completed. Sends without callbacks do not consume this capacity. When full, new sends with callbacks are rejected. Range: 1024–1048576 (`1024 * 1024`).                                                                                                                     |
| `max_attempts`          | `int`   | `10`                                   | Maximum sends per batch, including the first attempt. Must be at least 1; set to 1 to disable retries.                                                                                                                                                                                                                          |
| `base_backoff`          | `float` | `0.2` s                                | Initial upper bound on the retry wait. In seconds; range: 0.1–60; must not exceed `max_backoff`.                                                                                                                                                                                                                                |
| `max_backoff`           | `float` | `10.0` s                               | Maximum upper bound on the retry wait. In seconds; range: 0.1–600.                                                                                                                                                                                                                                                              |
| `delivery_timeout`      | `float` | `600.0` s                              | Soft delivery budget from when the batch stops accumulating (batch sealing), including queueing, processing, requests, and retries. Excludes accumulation (`linger`) and callbacks. Checked before processing or sending; in-flight requests use their own timeout and may finish later. In seconds; range: 60–604800 (7 days). |

All ranges are inclusive. A batch enters the sending flow when any of its size, count, or wait-time thresholds is reached.

For a short custom provider example, see [Dynamic credentials](credentials.md).

## Related documents

- [Quick start](quickstart.md)
- [Examples](examples.md)
- [Error handling](errors.md)
