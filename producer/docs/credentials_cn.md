# Rust 动态凭证

创建一个 struct，实现 `CredentialsProvider` 的异步方法 `fetch_credentials(&self)`，返回 `Result<Credentials, CredentialsError>`。使用 `#[async_trait]`，并通过 `with_credentials_provider()` 配置。

```rust,no_run
use aliyun_log_producer::{Credentials, CredentialsError, CredentialsProvider, ProducerConfig};
use aliyun_log_rust_sdk::async_trait;
use std::time::{Duration, SystemTime};

struct MyCredentialsProvider;

#[async_trait]
impl CredentialsProvider for MyCredentialsProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        // 在这里执行你的获取凭证逻辑，并转换为 Credentials；过期时间使用凭证的真实值。
        Ok(Credentials::new("access_key_id", "access_key_secret")?
            .with_security_token("sts_token")
            .with_expiration(SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000)))
    }
}

let config = ProducerConfig::default()
    .with_endpoint("cn-hangzhou.log.aliyuncs.com")
    .with_credentials_provider(MyCredentialsProvider);
```

创建 Producer 时不会获取凭证；首次发送请求时调用该方法，之后在临近过期且有请求时再次调用。返回的凭证由 SDK 自动缓存，无需额外缓存或定时刷新。

临时凭证应设置真实过期时间（`SystemTime`）。不使用 STS 时可省略 token。获取失败时返回 `CredentialsError`。

## 相关文档

- [快速开始](quickstart_cn.md)
