# Rust configuration

[简体中文](configuration_cn.md) · [Quick start](quickstart.md) · [Examples](examples.md) · [Error handling](errors.md)

Use the following code to create a Producer and send logs.

```rust
use aliyun_log_producer::{log, Producer, ProducerConfig};
use std::env;

let config = ProducerConfig::default()
    .with_endpoint("cn-hangzhou.log.aliyuncs.com")
    .with_access_key(
        env::var("ALIBABA_CLOUD_ACCESS_KEY_ID")?,
        env::var("ALIBABA_CLOUD_ACCESS_KEY_SECRET")?,
    );
let producer = Producer::create(config)?;
let writer = producer.writer("my-project", "my-logstore")?;
writer.send(log!("message": "hello"))?;

// Call before exiting to wait for pending logs.
producer.close_blocking()?;
```

If creation fails, see [error handling](errors.md).

## Credentials

Producer needs credentials to send logs. Choose one of these options.

### Static credentials

Use `with_access_key(id, secret)` to set the AccessKey ID and secret, as in the example above.

### Dynamic credentials

Use `with_credentials_provider(provider)` to configure dynamic credentials.
For temporary credentials, supply the expiration time so the SDK can refresh them automatically.

## Producer configuration options

| Setter | Type | Default | Meaning and range |
| --- | --- | --- | --- |
| `with_endpoint(endpoint)` | `impl Into<String>` | Required | SLS endpoint, for example `cn-hangzhou.log.aliyuncs.com`. |
| `with_user_agent(value)` | `impl Into<String>` | `aliyun-log-rust-producer/<version>` | Custom User-Agent sent with HTTP requests to identify your application. |
| `with_compression(value)` | `Compression` | `Compression::Zstd` | Log compression format: `Compression::Zstd` or `Compression::Lz4`. |
| `with_generate_pack_id(value)` | `bool` | `true` | Automatically add PackId to enable context queries after logs are written to the Logstore. Does not deduplicate logs. |
| `with_batch_size_threshold(value)` | `usize` | 1 MiB (`1048576`) | Accumulate logs across multiple `send` calls and send a batch when its estimated log size reaches this value. Range: 1–8388608 bytes (8 MiB); not a single-log size limit. |
| `with_batch_count_threshold(value)` | `usize` | `4096` | Accumulate logs across multiple `send` calls and send a batch when its log count reaches this value. Range: 1–40960. |
| `with_linger(value)` | `Duration` | 2 s | Maximum time logs are cached for accumulation; when elapsed, the batch immediately enters the sending flow. Range: 10 ms–365 days. |
| `with_buffer_bytes(value)` | `usize` | 128 MiB (`134217728`) | Estimated raw-data budget for pending logs. When full, new logs are rejected and your application can retry later. This is not a process memory limit. |
| `with_processing_workers(value)` | `usize` | `2` | Number of worker threads processing logs. Must be at least 1. |
| `with_callback_capacity(value)` | `usize` | `65536` | Maximum number of logs whose callbacks have not yet completed. Sends without callbacks do not consume this capacity. When full, new sends with callbacks are rejected. Range: 1024–1048576 (`1024 * 1024`). |
| `with_max_attempts(value)` | `u32` | `10` | Maximum sends per batch, including the first attempt. Must be at least 1; set to 1 to disable retries. |
| `with_base_backoff(value)` | `Duration` | 200 ms | Initial upper bound on the retry wait. Range: 100 ms–60 s; must not exceed `max_backoff`. |
| `with_max_backoff(value)` | `Duration` | 10 s | Maximum upper bound on the retry wait. Range: 100 ms–600 s. |
| `with_delivery_timeout(value)` | `Duration` | 600 s | Soft delivery budget from when the batch stops accumulating (batch sealing), including queueing, processing, requests, and retries. Excludes accumulation (`linger`) and callbacks. Checked before processing or sending; in-flight requests use their own timeout and may finish later. Range: 60 s–7 days. |

All ranges are inclusive. A batch enters the sending flow when any of its size, count, or wait-time thresholds is reached.
