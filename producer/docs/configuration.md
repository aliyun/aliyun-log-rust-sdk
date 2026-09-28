# Rust configuration

[简体中文](configuration_cn.md) · [Quick start](quickstart.md) · [Examples](examples.md)

Use the following code to create a Producer and send logs.

```rust
use aliyun_log_producer::{log, Producer, ProducerConfig};
use std::{env, time::Duration};

let config = ProducerConfig::default()
    .with_endpoint("cn-hangzhou.log.aliyuncs.com")
    .with_access_key(
        env::var("ALIBABA_CLOUD_ACCESS_KEY_ID")?,
        env::var("ALIBABA_CLOUD_ACCESS_KEY_SECRET")?,
    )
    .with_linger(Duration::from_millis(100))
    .with_delivery_timeout(Duration::from_secs(60));
let producer = Producer::create(config)?;
let writer = producer.writer("my-project", "my-logstore")?;
writer.send(log!("message": "hello"))?;

// Call before exiting to wait for pending logs.
producer.close_blocking()?;
```

Creating a Producer with invalid settings returns `ProducerError::Config`.

## Credentials

Producer needs credentials to send logs. Choose one of these options.

### Static credentials

Use `with_access_key(id, secret)` to set the AccessKey ID and secret, as in the example above.

### Dynamic credentials

Use `with_credentials_provider(provider)` to configure dynamic credentials.
For temporary credentials, supply the expiration time so the SDK can refresh them automatically.

## Producer configuration options

| Setter | Default | Meaning and range |
| --- | --- | --- |
| `with_endpoint(endpoint)` | Required | SLS endpoint, for example `cn-hangzhou.log.aliyuncs.com`. |
| `with_user_agent(value)` | `aliyun-log-rust-producer/<version>` | Custom User-Agent sent with HTTP requests to identify your application. |
| `with_compression(value)` | `Compression::Zstd` | Log compression format: `Compression::Zstd` or `Compression::Lz4`. |
| `with_generate_pack_id(value)` | `true` | Add PackId for log context queries. Does not deduplicate logs. |
| `with_batch_size_threshold(value)` | 1 MiB (`1048576`) | Send a batch when its estimated log size reaches this value. Range: 1–8388608 bytes (8 MiB). |
| `with_batch_count_threshold(value)` | `4096` | Send a batch when its log count reaches this value. Range: 1–40960. |
| `with_linger(value)` | 2 s | Maximum time to wait for more logs before sending. Zero disables the wait. at most 365 days. |
| `with_buffer_bytes(value)` | 128 MiB (`134217728`) | Buffer budget for pending logs. When full, new logs are rejected and your application can retry later. This is not a process memory limit. |
| `with_processing_workers(value)` | `4` | Number of worker threads processing logs. |
| `with_callback_capacity(value)` | `65536` | Maximum logs waiting for callbacks to finish. When full, new sends with callbacks are rejected. |
| `with_max_attempts(value)` | `10` | Maximum sends per batch, including the first attempt. Set to 1 to disable retries. |
| `with_base_backoff(value)` | 200 ms | Initial wait before retrying. Must not exceed `max_backoff`. must be greater than zero and at most 365 days. |
| `with_max_backoff(value)` | 10 s | Maximum wait between retries. Actual waits are randomized. must be greater than zero and at most 365 days. |
| `with_delivery_timeout(value)` | 600 s | Total time allowed to send a batch, including batching, requests, and retries. must be greater than zero and at most 365 days. |
