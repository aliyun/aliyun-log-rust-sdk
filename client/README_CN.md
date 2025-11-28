# aliyun-log-rust-sdk

[English](README.md) | 简体中文

这是用于访问阿里云日志服务的 Rust SDK。  
此 SDK 使用 [tokio](https://docs.rs/tokio/latest/tokio/) 作为异步运行时。

支持的 API 列表可参见 [api列表](../docs/api_cn.rst)。

## 快速开始

### 1. 创建客户端

```rust
use aliyun_log_rust_sdk::{Client, Config, FromConfig};

let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .access_key("access_key_id", "access_key_secret")
    .build()?;
let client = Client::from_config(config)?;
```

### 2. 发送请求

```rust
use chrono::Utc;

let now = Utc::now().timestamp();
let one_hour_ago = now - 3600;

let resp = client.get_logs("my-project", "my-logstore")
    .from(one_hour_ago)         // 开始时间（必需）
    .to(now)                    // 结束时间（必需）
    .query("level:ERROR")       // 查询语句，遵循查询语法
    .offset(0)                  // 从第一条日志开始
    .lines(100)                 // 返回最多 100 条日志
    .send()
    .await?;
```
