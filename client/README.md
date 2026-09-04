# Aliyun Log Rust SDK

English | [简体中文](README_CN.md)

This crate is rust sdk for access Aliyun Log Service.  
This SDK uses [tokio](https://docs.rs/tokio/latest/tokio/) as async runtime.  

Check all [supported APIs](../docs/api.rst) here.

## Quick Start

1. Create a client

```rust
use aliyun_log_rust_sdk::{Client, Config, FromConfig};
let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .access_key("access_key_id", "access_key_secret")
    .build()?;
let client = Client::from_config(config)?;
```

Endpoints without a scheme use HTTPS. Use an explicit `http://` endpoint only
for trusted local development. TLS defaults to rustls; enable the `native-tls`
feature if required.

Long-running applications can implement `CredentialsProvider` to refresh STS
credentials before every request attempt. Retry timing can be configured with
`RetryPolicy`.

2. Send a request

```rust
use aliyun_log_rust_sdk::GetLogsRequest;
use chrono::Utc;
let now = Utc::now().timestamp();
let one_hour_ago = now - 3600;
let resp = client.get_logs("my-project", "my-logstore")
    .from(one_hour_ago)         // Start time (required)
    .to(now)                    // End time (required)
    .query("level:ERROR")       // Filter for error logs only
    .offset(0)                  // Start from the first log
    .lines(100)                 // Return up to 100 logs
    .send()
    .await?;
```

## Consumer Library

Use `consumer::ConsumerWorker` for coordinated consumption with automatic shard
assignment, processing retries, periodic checkpoint commits, and graceful
shutdown. See the [`consumer` module documentation](https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/consumer/)
for a complete example.
