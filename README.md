# Rust SDK for Aliyun Log Service

English | [简体中文](README_CN.md)

This is Rust SDK for accessing Aliyun Log Service.

[![crates-badge](https://img.shields.io/crates/v/aliyun-log-rust-sdk.svg)](https://crates.io/crates/aliyun-log-rust-sdk)   ![mit-badge](https://img.shields.io/badge/license-MIT-blue.svg)  [![Ci](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/rust.yml/badge.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/rust.yml)

[API List](docs/api.rst) | [Docs](https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/)

## Quick Start

### 1. Add Dependency

Add this crate to your Cargo.toml using the following command:

```bash
cargo add aliyun-log-rust-sdk
```

### 2. Create a Client

```rust
use aliyun_log_rust_sdk::{Client, Config, FromConfig};

let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .access_key("access_key_id", "access_key_secret")
    .build()?;
let client = Client::from_config(config)?;
```

### 3. Write Logs

```rust
use aliyun_log_sdk_protobuf::{Log, LogGroup};

let mut log = Log::from_unixtime(chrono::Utc::now().timestamp() as u32);
log.add_content_kv("level", "info")
    .add_content_kv("message", "Application started");

let mut log_group = LogGroup::new();
log_group.add_log(log);

client.put_logs("my-project", "my-logstore")
    .log_group(log_group)
    .send()
    .await?;
```

### 4. Query Logs

```rust
use chrono::Utc;

let now = Utc::now().timestamp();
let one_hour_ago = now - 3600;

let resp = client.get_logs("my-project", "my-logstore")
    .from(one_hour_ago)
    .to(now)
    .query("level:ERROR")
    .offset(0)
    .lines(100)
    .send()
    .await?;
```

## Static Credentials Provider

Use `static_credentials_provider` to configure a fixed set of credentials.
The existing `.access_key()` and `.sts()` methods remain supported.

```rust
use aliyun_log_rust_sdk::{static_credentials_provider, Config};

let provider = static_credentials_provider("access_key_id", "access_key_secret", None)?;
// For STS credentials, use Some("sts_token".to_string()) instead of None.
let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .credentials_provider(provider)
    .build()?;
```

The helper validates the keys and creates nonexpiring credentials. To wrap an existing
`Credentials` value with optional expiration/update time, use
`StaticCredentialsProvider::new(credentials)`. A static provider cannot renew
temporary credentials when they expire.

## Dynamic Credentials

Implement `CredentialsProvider` to fetch credentials from your own asynchronous
source. The SDK re-exports `async_trait`, so no separate macro dependency is needed.

```rust
use aliyun_log_rust_sdk::{
    async_trait, Client, Config, Credentials, CredentialsError, CredentialsProvider, FromConfig,
};
use std::time::{Duration, SystemTime};

struct MyProvider;

#[async_trait]
impl CredentialsProvider for MyProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        // Replace with your asynchronous credentials-source call.
        // Convert its errors with .map_err(CredentialsError::provider)?;
        Ok(Credentials::new("access_key_id", "access_key_secret")?
            .with_security_token("sts_token") // optional
            .with_expiration(SystemTime::now() + Duration::from_secs(3600)) // optional
            .with_update_time(SystemTime::now())) // optional metadata
    }
}

let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .credentials_provider(MyProvider)
    .credentials_fetch_timeout(Duration::from_secs(5)) // default, per attempt
    .build()?;
let client = Client::from_config(config)?;
```

Both access keys must be nonempty. The STS token, expiration, and update time are
optional. Missing expiration means the credentials do not expire; update time is
metadata only. Return unexpired credentials from your provider.

The SDK manages credential refreshes automatically. If fetching fails, requests use
previously obtained credentials, **even if expired**. If none are available, the
request returns `Error::Credentials`.

Providers must support concurrent calls and cancellation-safe async I/O. Configure
the timeout for each fetch attempt with `.credentials_fetch_timeout()` (default:
5 seconds, must be nonzero), independently of the SLS HTTP request timeout.

`Arc<MyProvider>`, `Arc<dyn CredentialsProvider>`, and the clonable
`SharedCredentialsProvider` are accepted. Do not combine `.credentials_provider()`
with `.access_key()` or `.sts()`.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
