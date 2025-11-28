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

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
