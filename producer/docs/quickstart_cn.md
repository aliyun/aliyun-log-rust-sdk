# Rust 快速开始

[English](quickstart.md) · [概览](../README_CN.md) · [使用示例](examples_cn.md) · [配置参考](configuration_cn.md)

## 1. 添加依赖

在应用目录中执行：

```sh
cargo add aliyun-log-producer
```

## 2. 创建、发送和关闭

准备已有的 Project、Logstore，以及有写入权限的 AccessKey。
在环境中设置 `ALIBABA_CLOUD_ACCESS_KEY_ID` 和 `ALIBABA_CLOUD_ACCESS_KEY_SECRET`。
将 endpoint、project 和 logstore 替换为实际值，保存为 `src/main.rs`：

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

    // 程序运行期间复用 Producer，在程序退出前统一关闭，等待投递完成。
    producer.close_blocking()?;
    Ok(())
}
```

执行 `cargo run --release`。Producer 创建后自动启动，日志在后台发送。
