# Rust 使用示例

以下示例使用[快速开始](quickstart_cn.md)中创建的 Producer 和 writer。

## 选择发送方法

| 需要做什么               | 方法                                                     |
| ------------------------ | -------------------------------------------------------- |
| 发送日志                 | `send(log)`                                              |
| 设置来源和主题           | `send_with_options(log, options)`                        |
| 获取发送结果             | `send_with_callback(log, callback)`                      |
| 同时设置来源、主题和回调 | `send_with_options_and_callback(log, options, callback)` |

## 标记日志来源和主题

用 `source` 标记日志来自哪台机器，用 `topic` 区分业务，例如订单日志：

```rust
use aliyun_log_producer::{log, SendOptions};

writer.send_with_options(
    log!("level": "INFO", "message": "order created"),
    SendOptions::default().with_source("web-01").with_topic("orders"),
)?;
```

## 设置日志时间

`log!("key": "value", ...)` 自动使用当前时间。
可使用 `log!(time = event_time; "key": "value", ...)` 指定日志时间。

```rust
use aliyun_log_producer::log;
use std::time::{Duration, SystemTime};

let event_time = SystemTime::now() - Duration::from_secs(60);
writer.send(log!(time = event_time; "message": "imported log"))?;
```

## 确认日志是否发送成功

`send` 返回时，日志可能还没到达 SLS。使用 `send_with_callback`，可以收到发送成功或失败的通知：

```rust
use aliyun_log_producer::log;

writer.send_with_callback(log!("message": "hello"), |result| {
    match result {
        Ok(()) => println!("delivered"),
        Err(error) => eprintln!("failed: {error}; request_id={:?}", error.request_id()),
    }
})?;
```

如果发送方法直接返回错误，日志没有被接收，也不会触发回调。
回调应尽快返回。不要在回调中关闭或 flush 同一个 Producer。

## 等待发送完成与关闭

`flush_blocking()` 可能阻塞当前线程，仅在需要等待之前的日志发送结束时使用，无需每次发送后调用。

| 需要做什么                         | 调用方法                           |
| ---------------------------------- | ---------------------------------- |
| 等之前的日志发送结束，然后继续发送 | `flush_blocking()`，不等待回调     |
| 程序退出前，等日志发送和回调结束   | `close_blocking()`，之后不能再发送 |

```rust
producer.flush_blocking()?;
// 此处可以继续使用 writer。
producer.close_blocking()?;
```

在异步函数中使用 `flush().await` 和 `close().await`，等待时不会阻塞当前线程：

```rust
producer.flush().await?;
// 此处可以继续使用 writer。
producer.close().await?;
```

关闭前无需再调用 `flush`。这两个操作都不会报告单条日志的发送失败；需要确认结果时，请使用回调。

## 发送到多个 Logstore

使用同一个 SLS 服务地址和凭证时，可以共用一个 Producer：

```rust
use aliyun_log_producer::log;

let orders = producer.writer("my-project", "orders")?;
let audit = producer.writer("my-project", "audit")?;

let entry = log!("message": "order created");
orders.send(entry.clone())?;
audit.send(entry)?;
```

多个线程可以使用同一个 writer 的克隆，无需为每个线程创建 Producer。

## 发送太快，队列满了怎么办

收到 `ProducerError::EnqueueFull` 时，可以取出日志稍后重试。下面的示例等待 50 ms 后重试一次：

```rust
use aliyun_log_producer::{log, ProducerError};
use std::time::Duration;

match writer.send(log!("message": "hello")) {
    Ok(()) => {}
    Err(ProducerError::EnqueueFull { log }) => {
        std::thread::sleep(Duration::from_millis(50));
        writer.send(log)?;
    }
    Err(error) => return Err(error.into()),
}
```

第二次发送仍可能失败，需要由应用处理。使用回调时，重试也要传入回调。
异步代码应使用异步等待，例如 `tokio::time::sleep`。

## 相关文档

- [快速开始](quickstart_cn.md)
- [配置参考](configuration_cn.md)
- [错误处理](errors_cn.md)
- [回调用法](callbacks_cn.md)
