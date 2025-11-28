# 阿里云日志服务 Rust SDK

[English](README.md) | 简体中文

这里是阿里云日志服务官方 RUST SDK 项目。

[![crates-badge](https://img.shields.io/crates/v/aliyun-log-rust-sdk.svg)](https://crates.io/crates/aliyun-log-rust-sdk)  ![mit-badge](https://img.shields.io/badge/license-MIT-blue.svg) [![Ci](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/rust.yml/badge.svg)](https://github.com/aliyun/aliyun-log-rust-sdk/actions/workflows/rust.yml)

[API列表](docs/api_cn.rst) | [文档](https://docs.rs/tokio/latest/tokio)

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

## 贡献

欢迎贡献！请随时提交 Pull Request。
