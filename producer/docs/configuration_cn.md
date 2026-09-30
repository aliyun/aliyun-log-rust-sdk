# Rust 配置参考

可以参照下面代码创建 Producer，并写入日志。

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

| 方法                                | 类型                | 默认值                               | 说明与范围                                                                                                                                                                             |
| ----------------------------------- | ------------------- | ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `with_endpoint(endpoint)`           | `impl Into<String>` | 必填                                 | SLS 服务地址，例如 `cn-hangzhou.log.aliyuncs.com`。                                                                                                                                    |
| `with_user_agent(value)`            | `impl Into<String>` | `aliyun-log-rust-producer/<version>` | 自定义 HTTP 请求中的 User-Agent，用于标识应用。                                                                                                                                        |
| `with_compression(value)`           | `Compression`       | `Compression::Zstd`                  | 日志压缩方式，可选 `Compression::Zstd` 或 `Compression::Lz4`。                                                                                                                         |
| `with_generate_pack_id(value)`      | `bool`              | `true`                               | 自动添加 PackId，写入日志库后可进行上下文查询；不能用于日志去重。                                                                                                                      |
| `with_batch_size_threshold(value)`  | `usize`             | 1 MiB (`1048576`)                    | 多次调用 `send` 会进行攒批，每批日志的预估大小达到此值时发送。范围 1–8388608 字节（8 MiB），不是单条日志大小上限。                                                                     |
| `with_batch_count_threshold(value)` | `usize`             | `4096`                               | 多次调用 `send` 会进行攒批，每批日志达到此条数时发送。范围 1–40960。                                                                                                                   |
| `with_linger(value)`                | `Duration`          | 2 s                                  | 日志被缓存以进行攒批的最长时间，到时立即进入发送流程。范围 10 ms–365 天。                                                                                                              |
| `with_buffer_bytes(value)`          | `usize`             | 128 MiB (`134217728`)                | 待发送日志的预估原始数据量预算。达到后，新日志会被拒绝；需要由应用稍后重试。这不是进程内存上限。                                                                                       |
| `with_processing_workers(value)`    | `usize`             | `2`                                  | 处理日志的工作线程数，至少为 1。                                                                                                                                                       |
| `with_callback_capacity(value)`     | `usize`             | `65536`                              | 尚未完成回调的日志数量上限。不带回调的发送不占用此容量。达到后，带回调的新日志会被拒绝。范围 1024–1048576（`1024 * 1024`）。                                                           |
| `with_max_attempts(value)`          | `u32`               | `10`                                 | 每批日志最大发送次数，包含首次发送。至少为 1；1 表示不重试。                                                                                                                           |
| `with_base_backoff(value)`          | `Duration`          | 200 ms                               | 重试等待时间的初始上限。范围 100 ms–60 s，且不能超过 `max_backoff`。                                                                                                                   |
| `with_max_backoff(value)`           | `Duration`          | 10 s                                 | 重试等待时间的最大上限。范围 100 ms–600 s。                                                                                                                                            |
| `with_delivery_timeout(value)`      | `Duration`          | 600 s                                | 从批次停止聚合（封批）开始计算的投递软时限，包含排队、处理、请求和重试，不包含聚合等待（`linger`）和回调。处理或发送前检查；正在进行的请求使用自身超时，允许稍晚完成。范围 60 s–7 天。 |

以上范围均包含边界值。每批日志达到大小、条数或等待时间中的任一条件后进入发送流程。

自定义 provider 的简短示例见[动态凭证](credentials_cn.md)。

## 相关文档

- [快速开始](quickstart_cn.md)
- [使用示例](examples_cn.md)
- [错误处理](errors_cn.md)
