# Aliyun Log Producer for Rust

English | [简体中文](README_CN.md)

Async batching producer for Aliyun Log Service (SLS). Buffers records in memory, flushes by size/count/time, retries failures with exponential backoff, and supports backpressure, per-record acknowledgements, and runtime stats.

## Installation

```toml
[dependencies]
aliyun-log-rust-sdk-producer = { git = "https://github.com/aliyun/aliyun-log-rust-sdk" }
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
        .send(LogRecord::new(SystemTime::now())
            .field("level", "INFO")
            .field("message", "hello"))
        .await?;

    producer.close_and_wait().await?;
    Ok(())
}
```

## Sending APIs

| Method | Description |
| --- | --- |
| `send` | Enqueue one record. Returns when accepted, not when delivered. |
| `try_send` | Non-blocking enqueue. Returns `TrySendError` immediately on failure. |
| `send_with_ack` | Enqueue and get an `AckHandle`. Call `.wait().await` for the delivery result. |
| `send_many` | Enqueue a batch of records in one call. |
| `send_result` / `try_send_result` / `send_with_ack_result` | Same as above, but return the original `LogRecord` on failure. |

## Lifecycle

| Method | Description |
| --- | --- |
| `flush` | Flush the current partial batch and wait until all queued/inflight work is drained. |
| `flush_timeout` | Same as `flush`, but with a deadline. |
| `close` | Begin shutdown. Returns immediately; inflight batches may still be running. |
| `close_and_wait` | Shutdown and wait until all work is drained. |
| `close_timeout` | Same as `close_and_wait`, but with a deadline. |

## Builder Parameters

`endpoint`, credentials, `project`, and `logstore` are required when using the built-in SLS exporter.

| Parameter | Default | Description |
| --- | --- | --- |
| `endpoint` | required | SLS endpoint, e.g. `cn-hangzhou.log.aliyuncs.com` |
| `access_key` / `sts` | required | Credentials |
| `project` | required | Target project |
| `logstore` | required | Target logstore |
| `topic` | none | Topic for the `LogGroup` |
| `source` | none | Source for the `LogGroup` |
| `add_log_tag` | none | Static tags for every batch |
| `batch_max_events` | `256` | Max records per batch |
| `batch_max_bytes` | `512 KiB` | Max estimated bytes per batch |
| `linger` | `200ms` | Max wait before flushing a partial batch |
| `channel_capacity` | `1024` | Internal ingress queue capacity |
| `memory_limit_bytes` | `64 MiB` | Memory budget for queued + inflight records |
| `when_full` | `Block` | Backpressure policy: `Block` or `ReturnError` |
| `concurrency` | `4` | Max concurrent export requests |
| `export_timeout` | `5s` | Per-request timeout |
| `max_retries` | `3` | Max retries for retryable errors |
| `base_backoff` | `100ms` | Initial retry backoff |
| `max_backoff` | `3s` | Maximum retry backoff |
| `callback` | none | `ProducerCallback` for delivery events |
| `sink` | built-in SLS | Custom `LogSink` (mainly for tests) |

## Backpressure

| `WhenFull` | Behavior |
| --- | --- |
| `Block` | `send` waits until capacity is available |
| `ReturnError` | `send` / `try_send` return `MemoryLimitExceeded` or `QueueFull` |

## Retry Policy

| HTTP status | Behavior |
| --- | --- |
| 400, 401, 405 | Not retried (parameter/auth errors) |
| 403 | Retried with `max_backoff` as the floor delay (throttling) |
| 500+ | Retried with exponential backoff |
| Timeout / network | Retried with exponential backoff |

## Error Reference

**Build**: `BuildError::InvalidConfig` — missing required fields or zero-valued limits.

**Enqueue**: `SendError` / `TrySendError` — `Closed`, `QueueFull`, `MemoryLimitExceeded`, or `Encode(RecordError)`.

**Flush**: `FlushError` — `Closed` or `Timeout`.

**Close**: `CloseError` — `Timeout` or `Internal(...)`.

**Delivery** (via `AckHandle::wait()`): `DeliveryError` — `Timeout`, `Network`, `Server { retryable, throttled, .. }`, `RetriableExceeded`, `Shutdown`, or `Internal`.

## Callbacks

```rust
use aliyun_log_rust_sdk_producer::{DeliveryError, DeliveryReport, ProducerCallback};

struct MyCallback;

impl ProducerCallback for MyCallback {
    fn on_delivery(&self, report: &DeliveryReport) {
        println!("batch {} delivered: {} records", report.batch_id, report.record_count);
    }
    fn on_error(&self, error: &DeliveryError) {
        eprintln!("delivery failed: {error}");
    }
}
```

Callback panics are caught and logged.

## Stats

`producer.stats()` returns a `ProducerStats` snapshot:

| Field | Meaning |
| --- | --- |
| `queued_records` / `queued_bytes` | Records and bytes pending delivery |
| `inflight_batches` | Batches currently being exported |
| `accepted_records` | Total records accepted |
| `sent_records` / `sent_batches` | Successfully delivered |
| `failed_batches` | Terminal failures |
| `retry_count` | Total retry attempts |

## Shutdown

1. Stop producing new records.
2. Call `flush().await` to dispatch queued records immediately.
3. Call `close_and_wait().await` (or `close_timeout(...)`) before process exit.
