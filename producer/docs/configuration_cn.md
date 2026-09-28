# Rust 配置参考

[English](configuration.md) · [快速开始](quickstart_cn.md) · [使用示例](examples_cn.md) · [错误处理](errors_cn.md)

可以参照下面代码创建 Producer，并写入日志。

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

// 程序退出前调用，等待日志发送完成。
producer.close_blocking()?;
```

创建失败时，请参照[错误处理](errors_cn.md)。

## 配置凭证

Producer 需要凭证才能写入日志。可以选择以下两种方式。

### 静态凭证

通过 `with_access_key(id, secret)` 设置 AccessKey ID 和 AccessKey Secret，如上面的示例。

### 动态凭证

通过 `with_credentials_provider(provider)` 配置动态凭证。
使用临时凭证时，请提供到期时间，供 SDK 自动刷新凭证。

## Producer 配置选项

| 方法 | 类型 | 默认值 | 说明与范围 |
| --- | --- | --- | --- |
| `with_endpoint(endpoint)` | `impl Into<String>` | 必填 | SLS 服务地址，例如 `cn-hangzhou.log.aliyuncs.com`。 |
| `with_user_agent(value)` | `impl Into<String>` | `aliyun-log-rust-producer/<version>` | 自定义 HTTP 请求中的 User-Agent，用于标识应用。 |
| `with_compression(value)` | `Compression` | `Compression::Zstd` | 日志压缩方式，可选 `Compression::Zstd` 或 `Compression::Lz4`。 |
| `with_generate_pack_id(value)` | `bool` | `true` | 添加 PackId，便于查询日志上下文；不能用于日志去重。 |
| `with_batch_size_threshold(value)` | `usize` | 1 MiB (`1048576`) | 一批日志的预估大小达到此值时发送。范围 1–8388608 字节（8 MiB）。 |
| `with_batch_count_threshold(value)` | `usize` | `4096` | 一批日志的条数达到此值时发送。范围 1–40960。 |
| `with_linger(value)` | `Duration` | 2 s | 等待更多日志的最长时间，到时发送；0 表示不等待。不超过 365 天。 |
| `with_buffer_bytes(value)` | `usize` | 128 MiB (`134217728`) | 待发送日志的缓冲容量。达到后，新日志会被拒绝；需要由应用稍后重试。这不是进程内存上限。 |
| `with_processing_workers(value)` | `usize` | `2` | 处理日志的工作线程数。 |
| `with_callback_capacity(value)` | `usize` | `65536` | 最多允许多少条日志等待回调完成。达到后，带回调的新日志会被拒绝。 |
| `with_max_attempts(value)` | `u32` | `10` | 每批日志最多发送多少次，包含首次发送。1 表示不重试。 |
| `with_base_backoff(value)` | `Duration` | 200 ms | 首次重试前的等待时间，不能超过 `max_backoff`。必须大于 0，不超过 365 天。 |
| `with_max_backoff(value)` | `Duration` | 10 s | 重试之间的最长等待时间。必须大于 0，不超过 365 天。 |
| `with_delivery_timeout(value)` | `Duration` | 600 s | 每批日志允许的总发送时间，包含等待更多日志、发送请求和重试。必须大于 0，不超过 365 天。 |
