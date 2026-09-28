# Rust 配置参考

[English](configuration.md) · [快速开始](quickstart_cn.md) · [使用示例](examples_cn.md)

通过 `ProducerConfig::default().with_*()` 配置，在创建 Producer 时生效。
未设置的选项使用下表默认值；创建后不能修改正在运行的 Producer。
无效配置在 `Producer::create` 时返回 `ProducerError::Config`。

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

## 连接与凭证

| 方法 | 是否必需 | 说明 |
| --- | --- | --- |
| `with_endpoint(endpoint)` | 是 | SLS 地域 endpoint，例如 `cn-hangzhou.log.aliyuncs.com` |
| `with_access_key(id, secret)` | 与 provider 二选一 | 非空的 AccessKey ID 和 Secret |
| `with_credentials_provider(provider)` | 与固定密钥二选一 | 实现 `CredentialsProvider` 的凭证提供者，也可提供临时凭证 |

后调用的凭证设置方法覆盖前者。provider 返回 `Credentials`；临时凭证应携带到期时间，以便刷新。
相关凭证类型可直接从 `aliyun_log_producer` 引入。创建 Producer 时不会获取凭证。

## 发送与资源配置

| 方法 | 默认值 | 说明与范围 |
| --- | --- | --- |
| `with_user_agent(value)` | `aliyun-log-rust-producer/<version>` | HTTP User-Agent 请求头；覆盖默认值，必须是合法的 HTTP 请求头值。 |
| `with_compression(value)` | `Compression::Zstd` | 使用 `Compression::Zstd` 或 `Compression::Lz4`。 |
| `with_generate_pack_id(value)` | `true` | 添加 PackId，便于 SLS 日志上下文查询；不提供去重。 |
| `with_batch_size_threshold(value)` | 1 MiB (`1048576`) | 触发合批发送的预估日志字节数，范围 1–8388608 字节（8 MiB）。 |
| `with_batch_count_threshold(value)` | `4096` | 触发合批发送的日志条数，范围 1–40960。 |
| `with_linger(value)` | 2 s | 从一批中最早接收的日志开始，等待更多日志的最长时间；0 表示不等待。 |
| `with_buffer_bytes(value)` | 128 MiB (`134217728`) | 已接收但尚未完成投递日志的预估字节软限制。正整数，不是进程内存上限。 |
| `with_processing_workers(value)` | `4` | 用于准备待投递日志的工作线程数，正整数。 |
| `with_callback_capacity(value)` | `65536` | 已接收但回调尚未执行完的日志数量上限，含尚在投递中的日志。正整数；无回调的发送不占此额度。 |
| `with_max_attempts(value)` | `10` | 最多投递尝试次数，含首次请求。正整数；1 表示不重试。 |
| `with_base_backoff(value)` | 200 ms | 初始重试退避时间，必须大于 0 且不超过 `max_backoff`。 |
| `with_max_backoff(value)` | 10 s | 重试退避上限，实际等待带随机性。必须大于 0。 |
| `with_delivery_timeout(value)` | 600 s | 总投递期限，包含攒批、请求和重试等等待。必须大于 0。 |

时间参数使用 `std::time::Duration`；`linger` 可为 0，其余时间参数必须大于 0，均不得超过 365 天。

按条数、预估字节数或 `linger` 任一条件满足时触发发送。字节阈值不包含 source、topic 等批次元数据；
最后加入的日志可能使一批略超阈值。`linger` 仅约束攒批等待，不保证日志在该时间内送达。
`delivery_timeout` 从同一批最早接收的日志起计算，同批日志共用该期限；它不是 flush / close 的等待超时。
