# 阿里云日志服务 Rust Producer

[English](README.md) | 简体中文

面向阿里云日志服务（SLS）的异步批量 Producer。在内存中聚合日志，按大小/条数/时间触发发送，失败时指数退避重试，支持背压控制、逐条确认和运行时统计。

## 安装

```toml
[dependencies]
aliyun-log-rust-sdk-producer = { git = "https://github.com/aliyun/aliyun-log-rust-sdk" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 快速开始

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

## 发送接口

| 方法 | 说明 |
| --- | --- |
| `send` | 发送一条日志，入队成功即返回，不等待投递完成 |
| `try_send` | 非阻塞发送，队列满或内存不足时立即返回错误 |
| `send_with_ack` | 发送并获取 `AckHandle`，调用 `.wait().await` 等待投递结果 |
| `send_many` | 一次提交多条日志 |
| `send_result` / `try_send_result` / `send_with_ack_result` | 同上，失败时连同原始 `LogRecord` 一起返回 |

## 生命周期

| 方法 | 说明 |
| --- | --- |
| `flush` | 立即发送当前未满批次，等待所有排队和正在投递的任务完成 |
| `flush_timeout` | 带超时的 `flush` |
| `close` | 发起关闭，不再接收新日志后立即返回，发送中的批次可能仍在后台运行 |
| `close_and_wait` | 发起关闭并等待所有任务完成 |
| `close_timeout` | 带超时的 `close_and_wait` |

## 参数列表

使用内置 SLS exporter 时，`endpoint`、凭证、`project`、`logstore` 为必填。

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `endpoint` | 必填 | SLS 接入点，如 `cn-hangzhou.log.aliyuncs.com` |
| `access_key` / `sts` | 必填 | 认证信息 |
| `project` | 必填 | 目标 project |
| `logstore` | 必填 | 目标 logstore |
| `topic` | 无 | LogGroup 的 topic |
| `source` | 无 | LogGroup 的 source |
| `add_log_tag` | 无 | 为每个批次附加固定 tag |
| `batch_max_events` | `256` | 单批最大记录数 |
| `batch_max_bytes` | `512 KiB` | 单批最大估算字节数 |
| `linger` | `200ms` | 批次未满时的最长等待时间 |
| `channel_capacity` | `1024` | 内部入口队列容量 |
| `memory_limit_bytes` | `64 MiB` | 排队和发送中的日志可占用的总内存 |
| `when_full` | `Block` | 背压策略：`Block` 或 `ReturnError` |
| `concurrency` | `4` | 最大并发导出数 |
| `export_timeout` | `5s` | 单次请求超时 |
| `max_retries` | `3` | 可重试错误的最大重试次数 |
| `base_backoff` | `100ms` | 重试初始退避 |
| `max_backoff` | `3s` | 重试最大退避 |
| `callback` | 无 | 投递事件回调 |
| `sink` | 内置 SLS | 自定义 `LogSink`（主要用于测试） |

## 背压策略

| `WhenFull` | 行为 |
| --- | --- |
| `Block` | `send` 等待直到容量可用 |
| `ReturnError` | `send` / `try_send` 立即返回 `MemoryLimitExceeded` 或 `QueueFull` |

## 重试策略

| HTTP 状态码 | 行为 |
| --- | --- |
| 400、401、405 | 不重试（参数/认证/方法错误） |
| 403 | 以 `max_backoff` 为下限重试（限流） |
| 500+ | 指数退避重试 |
| 超时 / 网络错误 | 指数退避重试 |

## 错误速查

**构建**：`BuildError::InvalidConfig` — 缺少必填字段或数值参数为 0。

**入队**：`SendError` / `TrySendError` — `Closed`、`QueueFull`、`MemoryLimitExceeded` 或 `Encode(RecordError)`。

**Flush**：`FlushError` — `Closed` 或 `Timeout`。

**关闭**：`CloseError` — `Timeout` 或 `Internal(...)`。

**投递**（通过 `AckHandle::wait()`）：`DeliveryError` — `Timeout`、`Network`、`Server { retryable, throttled, .. }`、`RetriableExceeded`、`Shutdown` 或 `Internal`。

## 回调

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

回调中的 panic 会被捕获并记录日志，不会导致 producer 崩溃。

## 运行时统计

`producer.stats()` 返回 `ProducerStats` 快照：

| 字段 | 含义 |
| --- | --- |
| `queued_records` / `queued_bytes` | 待处理的记录数和字节数 |
| `inflight_batches` | 正在投递的批次数 |
| `accepted_records` | 累计接收记录数 |
| `sent_records` / `sent_batches` | 累计成功投递 |
| `failed_batches` | 累计最终失败 |
| `retry_count` | 累计重试次数 |

## 关闭流程

1. 停止生产新日志。
2. 调用 `flush().await` 立即发送排队日志。
3. 在进程退出前调用 `close_and_wait().await` 或 `close_timeout(...)`。
