# aliyun-log-rust-sdk
This crate is rust sdk for access Aliyun Log Service.  
This SDK uses [tokio](https://docs.rs/tokio/latest/tokio/) as async runtime.  

For more [Documents](https://crates.io/crates/aliyun-log-rust-sdk).

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
