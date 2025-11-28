# aliyun-log-sdk-sign

## Description

This crate is used to generate signature for aliyun log service.

For more [Documents](https://docs.rs/aliyun-log-sdk-sign).

## Quick Start

Add this crate to your Cargo.toml using the following command:

```bash
cargo add aliyun-log-sdk-sign
```

Use it in your code:

```rust
use aliyun_log_sdk_sign::{sign_v1, QueryParams};
let mut headers = http::HeaderMap::new();
let signature_result = sign_v1(
    "your_access_key_id",
    "your_access_key_secret",
    None,
    http::Method::GET,
    "/",
    &mut headers,
    QueryParams::empty(),
    None,
);
if let Err(err) = signature_result {
    println!("signature error: {}", err);
}

// with body, security token and query params
let signature_result = sign_v1(
    "your_access_key_id",
    "your_access_key_secret",
    Some("your_security_token"),
    &http::Method::POST,
    "/logstores/test-logstore/logs",
    &mut headers,
    [("key1", "value1"), ("key2", "value2")].into(),
    Some("HTTP body contents"),
);
if let Err(err) = signature_result {
    println!("signature error: {}", err);
}
```
