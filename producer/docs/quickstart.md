# Rust quick start

[简体中文](quickstart_cn.md) · [Overview](../README.md) · [Examples](examples.md) · [Configuration](configuration.md)

## 1. Add the dependency

In your application's `Cargo.toml`, adjust the path to your local checkout:

```toml
[dependencies]
aliyun-log-producer = { path = "../aliyun-log-rust-sdk/producer" }
```

## 2. Create, send and close

Use an existing Project and Logstore, with an AccessKey authorized to write to them.
Set `ALIBABA_CLOUD_ACCESS_KEY_ID` and `ALIBABA_CLOUD_ACCESS_KEY_SECRET` in your environment.
Replace the endpoint, project and logstore below, and save this as `src/main.rs`:

```rust,no_run
use aliyun_log_producer::{log, Producer, ProducerConfig};
use std::{env, time::SystemTime};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = ProducerConfig::default()
        .with_endpoint("cn-hangzhou.log.aliyuncs.com")
        .with_access_key(
            env::var("ALIBABA_CLOUD_ACCESS_KEY_ID")?,
            env::var("ALIBABA_CLOUD_ACCESS_KEY_SECRET")?,
        );
    let producer = Producer::create(config)?;
    let writer = producer.writer("my-project", "my-logstore")?;

    writer.send(log!("message": "hello SLS"))?;
    writer.send(log!(time = SystemTime::now(); "level": "INFO", "message": "another log"))?;

    // Reuse the producer while the application runs; close at shutdown to wait for delivery.
    producer.close_blocking()?;
    Ok(())
}
```

Run `cargo run --release`. Creating the producer starts it automatically.

A successful `send` means the log was accepted locally. Call `close_blocking()`
before exit to wait for pending delivery. In long-running applications, reuse the
producer and writer and close at shutdown.
