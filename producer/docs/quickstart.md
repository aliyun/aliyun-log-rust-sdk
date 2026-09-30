# Rust quick start

## 1. Add the dependency

Run in your application directory:

```sh
cargo add aliyun-log-producer
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

Run `cargo run --release`. The Producer starts automatically and sends logs in the background.

## Related documents

- [Examples](examples.md)
- [Configuration](configuration.md)
