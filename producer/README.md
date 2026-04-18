# Aliyun Log Producer for Rust

English | [简体中文](README_CN.md)

`aliyun-log-rust-sdk-producer` is an async producer for Aliyun Log Service (SLS).
It batches records in memory, flushes by size or time, retries retryable failures,
and provides backpressure, acknowledgements, callbacks, and runtime stats.

## Installation

### crates.io

```bash
cargo add aliyun-log-rust-sdk-producer
cargo add aliyun-log-rust-sdk
cargo add tokio --features macros,rt-multi-thread
```

### Git dependency

```toml
[dependencies]
aliyun-log-rust-sdk = { git = "https://github.com/aliyun/aliyun-log-rust-sdk" }
aliyun-log-rust-sdk-producer = { git = "https://github.com/aliyun/aliyun-log-rust-sdk", package = "aliyun-log-rust-sdk-producer" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick Start

```rust
use std::time::SystemTime;

use aliyun_log_rust_sdk_producer::{LogRecord, Producer};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let producer = Producer::builder()
        .endpoint("cn-hangzhou.log.aliyuncs.com")
        .access_key("access_key_id", "access_key_secret")
        .project("my-project")
        .logstore("my-logstore")
        .build()
        .await?;

    producer
        .send(
            LogRecord::new(SystemTime::now())
                .field("level", "INFO")
                .field("message", "producer started"),
        )
        .await?;

    producer.close_and_wait().await?;
    Ok(())
}
```

## Core Concepts

- `LogRecord`: a single log event with a timestamp and key/value fields.
- `Producer`: an async batching writer that accepts records and exports them in the background.
- `AckHandle`: returned by `send_with_ack`; resolves when the batch succeeds or fails permanently.
- `WhenFull`: controls backpressure behavior when memory or queue capacity is exhausted.
- `ProducerStats`: cheap runtime snapshot for queued, inflight, sent, failed, and retried data.

## Sending APIs

### `send`

Enqueue one record and return when it is accepted into the internal pipeline.
It does not mean the record has already been delivered to SLS.

### `send_with_ack`

Enqueue one record and receive an `AckHandle`.
Call `wait().await` on the handle when you need delivery success or the final `DeliveryError`.

### `send_result` / `try_send_result` / `send_with_ack_result`

These variants preserve the original `LogRecord` when enqueue fails.
Use them when the caller needs to retry, reroute, or persist the rejected record explicitly.

### `try_send`

Non-blocking enqueue. It returns immediately with `TrySendError` when the queue is full,
memory is exhausted, or the producer is closed.

### `send_many`

Enqueue multiple records in one call. This is useful when the caller already has a batch of records.

### `flush`

Force the current partial batch to be dispatched and wait until queued and inflight work is drained.

### `close`

Start graceful shutdown and return after the producer stops accepting new records.
Queued and inflight batches may still be finishing in the background.

### `close_and_wait` / `close_timeout`

Start graceful shutdown and wait until queued and inflight work is fully drained.
Call one of these before shutdown when you need graceful completion.

## Builder Parameters

When using the built-in SLS exporter, `endpoint`, credentials, `project`, and `logstore` are required.

| Parameter | Default | Description |
| --- | --- | --- |
| `endpoint(String)` | required | SLS endpoint, such as `cn-hangzhou.log.aliyuncs.com`. |
| `access_key(id, secret)` / `sts(id, secret, token)` | required | Credentials for the built-in SLS exporter. |
| `project(String)` | required | Target SLS project. |
| `logstore(String)` | required | Target SLS logstore. |
| `topic(String)` | none | Optional topic written into the `LogGroup`. |
| `source(String)` | none | Optional source written into the `LogGroup`. |
| `add_log_tag(key, value)` | none | Adds static tags to every exported batch. |
| `batch_max_events(usize)` | `256` | Flush when a batch reaches this many records. Must be `> 0`. |
| `batch_max_bytes(usize)` | `512 * 1024` | Flush when a batch reaches this estimated encoded size. Must be `> 0`. |
| `linger(Duration)` | `200ms` | Flush a partial batch after this delay. |
| `channel_capacity(usize)` | `1024` | Internal ingress channel capacity. Must be `> 0`. |
| `memory_limit_bytes(usize)` | `64 MiB` | Total estimated bytes allowed in queued + inflight records. Must be `> 0`. |
| `when_full(WhenFull)` | `WhenFull::Block` | Backpressure policy when queue or memory is saturated. |
| `concurrency(usize)` | `4` | Maximum concurrent export requests. Must be `> 0`. |
| `export_timeout(Duration)` | `5s` | Timeout for one export request attempt. |
| `max_retries(usize)` | `3` | Maximum retry attempts for retryable delivery failures. |
| `base_backoff(Duration)` | `100ms` | Initial retry backoff. |
| `max_backoff(Duration)` | `3s` | Maximum retry backoff. |
| `callback(Arc<dyn ProducerCallback>)` | none | Optional callback for delivery success and failure. |
| `sink(Arc<dyn LogSink>)` | built-in SLS exporter | Replace the exporter, mainly for tests or custom integrations. |

## Backpressure and Full-Queue Behavior

`WhenFull` defines what happens when `memory_limit_bytes` or the internal channel becomes the bottleneck:

| Mode | Behavior |
| --- | --- |
| `WhenFull::Block` | `send` waits until capacity becomes available. |
| `WhenFull::ReturnError` | `send` / `try_send` return `MemoryLimitExceeded` or `QueueFull`. |

Choose `WhenFull::Block` when you prefer backpressure, or `WhenFull::ReturnError` when you want the caller to handle overload explicitly.

## Error Handling

### Build-time errors

`build().await` returns `BuildError`:

| Error | Meaning |
| --- | --- |
| `InvalidConfig(...)` | A required builder field is missing, or one of the required numeric limits was set to `0`. |

### Enqueue-time errors

`send` and `try_send` can fail before a record enters the pipeline.
Use `send_result` / `try_send_result` / `send_with_ack_result` when you want the error together with the original record.

| Error | Meaning |
| --- | --- |
| `Closed` | The producer is already closing or closed. |
| `QueueFull` | `try_send` hit a full channel. |
| `MemoryLimitExceeded` | The estimated in-memory record budget was exceeded. |
| `Encode(RecordError::EmptyKey)` | A field key was empty. |
| `Encode(RecordError::RecordTooLarge)` | One record exceeded the 3 MiB encoded-size limit. |

### Flush and close errors

| API | Errors |
| --- | --- |
| `flush()` | `FlushError::Closed`, `FlushError::Internal(...)` |
| `close()` | `CloseError::Internal(...)` if shutdown could not be initiated |
| `close_and_wait()` | `CloseError::Internal(...)` |
| `close_timeout()` | `CloseError::Timeout` if graceful shutdown takes too long |

### Delivery errors

`AckHandle::wait()` returns a final `DeliveryError` if the batch was not delivered:

| Error | Meaning |
| --- | --- |
| `Timeout` | A request attempt timed out. Retryable. |
| `Network(...)` | A transport-level error occurred. Retryable. |
| `Server { retryable, .. }` | SLS returned a server error. Retry behavior depends on `retryable`. |
| `RetriableExceeded { last_error }` | Retries were exhausted for a retryable error. |
| `Shutdown` | The producer shut down before delivery completed. |
| `Internal(...)` | Unexpected internal or client-side error. |

The producer currently treats timeouts, network errors, and HTTP `500..=503` server responses as retryable.

## Delivery Callbacks

Implement `ProducerCallback` to observe producer behavior:

```rust
use aliyun_log_rust_sdk_producer::{DeliveryError, DeliveryReport, ProducerCallback};

struct MetricsCallback;

impl ProducerCallback for MetricsCallback {
    fn on_delivery(&self, report: &DeliveryReport) {
        println!("sent batch {} with {} records", report.batch_id, report.record_count);
    }

    fn on_error(&self, error: &DeliveryError) {
        eprintln!("delivery failed: {error}");
    }
}
```

Callback panics are caught and logged, so they do not crash the producer.

## Runtime Stats

Call `producer.stats()` at any time to obtain a snapshot:

| Field | Meaning |
| --- | --- |
| `queued_records` | Records accepted but not fully drained yet. |
| `queued_bytes` | Estimated bytes for queued + inflight records. |
| `inflight_batches` | Batches currently being exported. |
| `accepted_records` | Total records accepted by the producer. |
| `sent_records` | Total records delivered successfully. |
| `sent_batches` | Total successful export batches. |
| `failed_batches` | Total batches that finished with terminal delivery failure. |
| `retry_count` | Total retry attempts across all batches. |

## Recommended Shutdown Sequence

1. Stop producing new records.
2. Call `flush().await` if you want all queued records dispatched immediately.
3. Call `close_and_wait().await` or `close_timeout(...)` before process exit.
4. If you use `send_with_ack`, await outstanding ack handles before dropping the runtime.

## Notes

- `send()` only guarantees enqueue, not remote durability.
- `send_result()` / `try_send_result()` are the lossless enqueue APIs when rejected records must be retained by the caller.
- `send_with_ack()` is the right API when you need per-record delivery confirmation.
- `topic`, `source`, and `log_tags` are attached at the batch level.
- `sink(...)` lets you plug in a custom exporter, but most users should keep the default SLS exporter.
