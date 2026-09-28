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

Producer needs credentials to send logs. Choose one of the following two options.

### Static credentials

Use `with_access_key(id, secret)` to set the AccessKey ID and secret, as in the example above.
Neither value can be empty.

### Dynamic credentials

Use `with_credentials_provider(provider)` to configure dynamic credentials.
The provider must implement `CredentialsProvider` and return `Credentials`.
For temporary credentials, supply the expiration time so the SDK can refresh them automatically.

Import `CredentialsProvider` and `Credentials` from `aliyun_log_producer`.

## Producer configuration options

| Setter | Default | Meaning and range |
| --- | --- | --- |
| `with_endpoint(endpoint)` | Required | SLS endpoint, for example `cn-hangzhou.log.aliyuncs.com`. |
| `with_user_agent(value)` | `aliyun-log-rust-producer/<version>` | User-Agent sent with HTTP requests. Set it to override the default; must be a valid HTTP header value. |
| `with_compression(value)` | `Compression::Zstd` | Log compression format: `Compression::Zstd` or `Compression::Lz4`. |
| `with_generate_pack_id(value)` | `true` | Add PackId for log context queries. Does not deduplicate logs. |
| `with_batch_size_threshold(value)` | 1 MiB (`1048576`) | Send a batch when its estimated log size reaches this value. Range: 1–8388608 bytes (8 MiB). |
| `with_batch_count_threshold(value)` | `4096` | Send a batch when its log count reaches this value. Range: 1–40960. |
| `with_linger(value)` | 2 s | Maximum time to wait for more logs before sending. Zero disables the wait. Use `Duration`; at most 365 days. |
| `with_buffer_bytes(value)` | 128 MiB (`134217728`) | Budget for estimated bytes of logs awaiting delivery. Must be a positive integer; usage may slightly exceed it. This is not a process memory limit. |
| `with_processing_workers(value)` | `4` | Number of worker threads processing logs. Must be a positive integer. |
| `with_callback_capacity(value)` | `65536` | Maximum number of logs waiting for their callbacks to finish. Must be a positive integer. Logs without callbacks do not count. |
| `with_max_attempts(value)` | `10` | Maximum sends per batch, including the first attempt. Must be a positive integer; 1 disables retries. |
| `with_base_backoff(value)` | 200 ms | Initial wait before retrying. Must not exceed `max_backoff`. Use `Duration`; must be greater than zero and at most 365 days. |
| `with_max_backoff(value)` | 10 s | Maximum wait between retries. Actual waits are randomized. Use `Duration`; must be greater than zero and at most 365 days. |
| `with_delivery_timeout(value)` | 600 s | Total time allowed to send a batch, including batching, requests, and retries. Use `Duration`; must be greater than zero and at most 365 days. |
