# Rust configuration

[简体中文](configuration_cn.md) · [Quick start](quickstart.md) · [Examples](examples.md)

Configure with `ProducerConfig::default().with_*()` before creation. Omitted options
use the defaults below; running producers cannot be reconfigured.
`Producer::create` returns `ProducerError::Config` for invalid settings.

```rust
use aliyun_log_producer::{ProducerConfig, Producer};
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
producer.close_blocking()?;
```

## Connection and credentials

| Setter | Required | Meaning |
| --- | --- | --- |
| `with_endpoint(endpoint)` | Yes | Regional SLS endpoint, e.g. `cn-hangzhou.log.aliyuncs.com` |
| `with_access_key(id, secret)` | Either this or a provider | Nonempty AccessKey ID and secret |
| `with_credentials_provider(provider)` | Either this or fixed keys | A `CredentialsProvider` for obtaining credentials, including temporary credentials |

The last credentials setter wins. A provider returns `Credentials` and should supply
expiration for temporary credentials so they can be refreshed. The credentials types
are re-exported by `aliyun_log_producer`. Creation does not fetch credentials.

## Delivery and resource settings

| Setter | Default | Meaning and range |
| --- | --- | --- |
| `with_user_agent(value)` | `aliyun-log-rust-producer/<version>` | HTTP User-Agent header; replaces the default. Must be a valid HTTP header value. |
| `with_compression(value)` | `Compression::Zstd` | Use `Compression::Zstd` or `Compression::Lz4`. |
| `with_generate_pack_id(value)` | `true` | Add PackId for SLS log context queries. Does not deduplicate logs. |
| `with_batch_size_threshold(value)` | 1 MiB (`1048576`) | Estimated log bytes that trigger a batch send. Range: 1–8388608 bytes (8 MiB). |
| `with_batch_count_threshold(value)` | `4096` | Log count that triggers a batch send. Range: 1–40960. |
| `with_linger(value)` | 2 s | Maximum wait to collect more logs, starting with the oldest log in a batch. Zero disables this wait. |
| `with_buffer_bytes(value)` | 128 MiB (`134217728`) | Soft budget for estimated bytes of accepted logs still awaiting delivery. Positive integer; not a process memory limit. |
| `with_processing_workers(value)` | `4` | Number of workers preparing logs for delivery. Positive integer. |
| `with_callback_capacity(value)` | `65536` | Maximum accepted logs whose callbacks have not finished, including those still in delivery. Positive integer; sends without callbacks do not consume this capacity. |
| `with_max_attempts(value)` | `10` | Maximum delivery attempts including the first request. Positive integer; 1 disables retries. |
| `with_base_backoff(value)` | 200 ms | Initial retry backoff. Must be positive and no greater than `max_backoff`. |
| `with_max_backoff(value)` | 10 s | Maximum retry backoff; actual delays are randomized. Must be positive. |
| `with_delivery_timeout(value)` | 600 s | Total delivery deadline, including batching, requests and retries. Must be positive. |

Time settings use `std::time::Duration`. Linger may be zero; the other durations must be positive. All are limited to 365 days.

A batch is sent when its log count, estimated bytes or linger condition is met.
The byte threshold excludes group metadata such as source and topic; the last log
may take a batch beyond the threshold. Linger only limits batching delay, not delivery latency.
The delivery deadline starts at the oldest accepted log in a batch and is shared by
that batch; it is not a wait timeout for flush or close.
