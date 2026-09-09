# 阿里云日志服务 Rust SDK

[English](README.md) | 简体中文

这里是阿里云日志服务官方 RUST SDK 项目。

[![crates-badge](https://img.shields.io/crates/v/aliyun-log-rust-sdk.svg)](https://crates.io/crates/aliyun-log-rust-sdk)  ![mit-badge](https://img.shields.io/badge/license-MIT-blue.svg) [![Ci](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/rust.yml/badge.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/rust.yml)

[API列表](docs/api_cn.rst) | [文档](https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/)

## 快速开始

### 1. 添加依赖

使用以下命令将此 crate 添加到你的 Cargo.toml：

```bash
cargo add aliyun-log-rust-sdk
```

### 2. 创建客户端

```rust
use aliyun_log_rust_sdk::{Client, Config, FromConfig};

let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .access_key("access_key_id", "access_key_secret")
    .build()?;
let client = Client::from_config(config)?;
```

### 3. 写入日志

```rust
use aliyun_log_sdk_protobuf::{Log, LogGroup};

let mut log = Log::from_unixtime(chrono::Utc::now().timestamp() as u32);
log.add_content_kv("level", "info")
    .add_content_kv("message", "应用启动");

let mut log_group = LogGroup::new();
log_group.add_log(log);

client.put_logs("my-project", "my-logstore")
    .log_group(log_group)
    .send()
    .await?;
```

### 4. 查询日志

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

## 静态凭证 Provider

使用 `static_credentials_provider` 配置固定凭证，也可以继续使用原有的
`.access_key()`、`.sts()` 接口。

```rust
use aliyun_log_rust_sdk::{static_credentials_provider, Config};

let provider = static_credentials_provider("access_key_id", "access_key_secret", None)?;
// STS 凭证将 None 换成 Some("sts_token".to_string())。
let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .credentials_provider(provider)
    .build()?;
```

Helper 会校验 AK，创建不含过期时间的凭证。如果已有 `Credentials`，可通过
`StaticCredentialsProvider::new(credentials)` 保留其过期时间和更新时间。
静态 provider 不会自动续期临时凭证。

## 动态凭证

实现 `CredentialsProvider` 即可接入自定义异步凭证来源。SDK 已导出 `async_trait`，
无需另行添加宏依赖。

```rust
use aliyun_log_rust_sdk::{
    async_trait, Client, Config, Credentials, CredentialsError, CredentialsProvider, FromConfig,
};
use std::time::{Duration, SystemTime};

struct MyProvider;

#[async_trait]
impl CredentialsProvider for MyProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        // 替换为你的异步凭证获取调用。
        // 自定义错误可通过 .map_err(CredentialsError::provider)? 转换。
        Ok(Credentials::new("access_key_id", "access_key_secret")?
            .with_security_token("sts_token") // 可选
            .with_expiration(SystemTime::now() + Duration::from_secs(3600)) // 可选
            .with_update_time(SystemTime::now())) // 可选元数据
    }
}

let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .credentials_provider(MyProvider)
    .credentials_fetch_timeout(Duration::from_secs(5)) // 默认值，每次尝试的超时
    .build()?;
let client = Client::from_config(config)?;
```

AK ID 和 Secret 必填且不能为空。STS Token、过期时间和更新时间均可选。
缺失 `expiration` 表示凭证无限期有效；`update_time` 仅作为元数据。
Provider 应返回尚未过期的凭证。

SDK 自动管理凭证刷新。获取失败时，请求会使用之前取得的凭证，**即使它已过期**；
没有可用凭证时返回 `Error::Credentials`。

Provider 必须支持并发调用，并使用支持取消的异步 I/O。可通过
`.credentials_fetch_timeout()` 配置每次获取的超时（默认 5 秒，必须为非零时长），
与 SLS HTTP 请求超时分别设置。

支持传入 `Arc<MyProvider>`、`Arc<dyn CredentialsProvider>`，也提供可克隆的
`SharedCredentialsProvider`。`.credentials_provider()` 不能与 `.access_key()`
或 `.sts()` 混用。

## 贡献

欢迎贡献！请随时提交 Pull Request。
