# Aliyun Log Producer Rust 用户指南

[English](README.md) | 简体中文

`aliyun-log-rust-sdk-producer` 是阿里云日志服务（SLS）的异步 Producer。
它负责在内存中聚合日志、按大小或时间触发发送、对可重试错误自动重试，
并提供背压控制、投递确认、回调以及运行时统计能力。

## 安装方法

### 从 crates.io 安装

```bash
cargo add aliyun-log-rust-sdk-producer
cargo add aliyun-log-rust-sdk
cargo add tokio --features macros,rt-multi-thread
```

### 使用 Git 依赖

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

## 核心概念

- `LogRecord`：单条日志，包含时间戳和若干键值字段。
- `Producer`：异步批量写入器，后台负责聚合和发送日志。
- `AckHandle`：由 `send_with_ack` 返回，用于等待最终投递结果。
- `WhenFull`：当内存或队列容量达到上限时的背压策略。
- `ProducerStats`：轻量级运行时快照，可查看排队、投递、失败和重试情况。

## 发送接口

### `send`

发送单条日志，并在日志进入内部流水线后返回。
它只表示“已入队”，不表示“已经成功写入 SLS”。

### `send_with_ack`

发送单条日志并返回 `AckHandle`。
如果你需要确认最终是否投递成功，应调用 `wait().await`。

### `send_result` / `try_send_result` / `send_with_ack_result`

这些变体会在入队失败时把原始 `LogRecord` 一起返回。
当调用方需要自行重试、转存或旁路处理失败记录时，应优先使用它们。

### `try_send`

非阻塞发送。如果队列已满、内存额度不足，或者 producer 已关闭，会立即返回 `TrySendError`。

### `send_many`

一次性提交多条日志，适合调用方已经提前聚合好一批记录的场景。

### `flush`

主动触发当前未满批次发送，并等待队列和正在投递的任务全部清空。

### `close`

发起优雅关闭，并在 producer 不再接收新日志后立即返回。
此时队列中和发送中的批次仍可能在后台继续完成。

### `close_and_wait` / `close_timeout`

发起优雅关闭，并等待队列中和发送中的任务全部处理完成。
如果你需要在进程退出前确保发送完成，应使用其中之一。

## 参数列表

使用内置 SLS exporter 时，`endpoint`、凭证、`project`、`logstore` 是必填项。

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `endpoint(String)` | 必填 | SLS endpoint，例如 `cn-hangzhou.log.aliyuncs.com`。 |
| `access_key(id, secret)` / `sts(id, secret, token)` | 必填 | 内置 SLS exporter 使用的认证信息。 |
| `project(String)` | 必填 | 目标 SLS project。 |
| `logstore(String)` | 必填 | 目标 SLS logstore。 |
| `topic(String)` | 无 | 写入 `LogGroup` 的可选 topic。 |
| `source(String)` | 无 | 写入 `LogGroup` 的可选 source。 |
| `add_log_tag(key, value)` | 无 | 为所有批次附加固定 tag。 |
| `batch_max_events(usize)` | `256` | 单批最多记录数，达到后立即发送。必须 `> 0`。 |
| `batch_max_bytes(usize)` | `512 * 1024` | 单批估算字节数上限，达到后立即发送。必须 `> 0`。 |
| `linger(Duration)` | `200ms` | 批次未满时，最长等待这么久后也会发送。 |
| `channel_capacity(usize)` | `1024` | 内部入口队列容量。必须 `> 0`。 |
| `memory_limit_bytes(usize)` | `64 MiB` | 排队中和发送中的日志可占用的估算总内存。必须 `> 0`。 |
| `when_full(WhenFull)` | `WhenFull::Block` | 内存或队列打满时的处理策略。 |
| `concurrency(usize)` | `4` | 最多并发导出请求数。必须 `> 0`。 |
| `export_timeout(Duration)` | `5s` | 单次导出请求超时时间。 |
| `max_retries(usize)` | `3` | 可重试错误的最大重试次数。 |
| `base_backoff(Duration)` | `100ms` | 重试初始退避时间。 |
| `max_backoff(Duration)` | `3s` | 重试最大退避时间。 |
| `callback(Arc<dyn ProducerCallback>)` | 无 | 可选回调，用于观测成功和失败事件。 |
| `sink(Arc<dyn LogSink>)` | 内置 SLS exporter | 自定义导出器，常用于测试或特殊集成。 |

## 队列打满与背压策略

`WhenFull` 控制 `memory_limit_bytes` 或内部队列成为瓶颈时的行为：

| 模式 | 行为 |
| --- | --- |
| `WhenFull::Block` | `send` 会等待直到容量恢复。 |
| `WhenFull::ReturnError` | `send` / `try_send` 立即返回 `MemoryLimitExceeded` 或 `QueueFull`。 |

如果你希望 producer 主动施加背压，使用 `WhenFull::Block`；如果你希望由调用方显式处理过载，使用 `WhenFull::ReturnError`。

## 错误处理

### 构建阶段错误

`build().await` 可能返回 `BuildError`：

| 错误 | 含义 |
| --- | --- |
| `InvalidConfig(...)` | 缺少必填 builder 字段，或某个必须大于 0 的数值参数被设置为 0。 |

### 入队阶段错误

`send` 和 `try_send` 会在日志进入流水线前做校验和容量检查。
如果你希望在失败时拿回原始记录，请使用 `send_result` / `try_send_result` / `send_with_ack_result`。

| 错误 | 含义 |
| --- | --- |
| `Closed` | producer 已经开始关闭或已关闭。 |
| `QueueFull` | `try_send` 遇到内部队列已满。 |
| `MemoryLimitExceeded` | 估算内存额度不足。 |
| `Encode(RecordError::EmptyKey)` | 某个字段 key 为空。 |
| `Encode(RecordError::RecordTooLarge)` | 单条日志估算编码体积超过 3 MiB。 |

### flush / close 错误

| 接口 | 可能错误 |
| --- | --- |
| `flush()` | `FlushError::Closed`、`FlushError::Internal(...)` |
| `close()` | 如果无法成功发起关闭，返回 `CloseError::Internal(...)` |
| `close_and_wait()` | `CloseError::Internal(...)` |
| `close_timeout()` | 超时时返回 `CloseError::Timeout` |

### 投递阶段错误

`AckHandle::wait()` 在批次最终失败时返回 `DeliveryError`：

| 错误 | 含义 |
| --- | --- |
| `Timeout` | 单次请求超时。可重试。 |
| `Network(...)` | 网络错误。可重试。 |
| `Server { retryable, .. }` | 服务端错误，是否重试取决于 `retryable`。 |
| `RetriableExceeded { last_error }` | 可重试错误在耗尽重试次数后返回的终态错误。 |
| `Shutdown` | producer 在投递完成前已关闭。 |
| `Internal(...)` | 内部错误或客户端侧异常。 |

当前实现会把超时、网络错误，以及 HTTP `500..=503` 的服务端错误视为可重试错误。

## 回调与监控

你可以实现 `ProducerCallback` 来接收状态通知：

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

回调中的 panic 会被捕获并记录日志，不会直接把 producer 进程打崩。

## 运行时统计

调用 `producer.stats()` 可以获取快照：

| 字段 | 含义 |
| --- | --- |
| `queued_records` | 已被接收但还未完全处理完成的记录数。 |
| `queued_bytes` | 当前排队和发送中的估算字节数。 |
| `inflight_batches` | 正在投递的批次数。 |
| `accepted_records` | 已被 producer 接收的总记录数。 |
| `sent_records` | 成功投递的总记录数。 |
| `sent_batches` | 成功投递的总批次数。 |
| `failed_batches` | 最终失败的批次数。 |
| `retry_count` | 所有批次累计发生的重试次数。 |

## 推荐关闭流程

1. 停止继续生产新日志。
2. 如需尽快把排队日志发出去，先调用 `flush().await`。
3. 在进程退出前调用 `close_and_wait().await` 或 `close_timeout(...)`。
4. 如果使用了 `send_with_ack`，在 runtime 结束前等待所有 ack 完成。

## 使用建议

- `send()` 只保证“入队成功”，不保证“远端持久化成功”。
- 需要在入队失败时拿回原始记录时，请使用 `send_result()` / `try_send_result()`。
- 需要单条日志最终投递结果时，请使用 `send_with_ack()`。
- `topic`、`source` 和 `log_tags` 是按批次附加的，不是逐条单独配置。
- `sink(...)` 适合测试或自定义导出链路；大多数业务场景直接使用默认 SLS exporter 即可。
