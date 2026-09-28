# Rust 使用示例

[English](examples.md) · [快速开始](quickstart_cn.md) · [配置参考](configuration_cn.md)

以下片段沿用快速开始中的 `producer` 和 `writer`。将含 `?` 的片段放在返回
`Result<(), ProducerError>` 的函数中，并在应用退出时关闭 Producer。

## 四种发送方式

| 方法 | source / topic | 投递回调 |
| --- | --- | --- |
| `send(log)` | 默认空值 | 无 |
| `send_with_options(log, options)` | 自定义 | 无 |
| `send_with_callback(log, callback)` | 默认空值 | 有 |
| `send_with_options_and_callback(log, options, callback)` | 自定义 | 有 |

四种方法都立即接收或返回错误，不等待投递完成。

## 设置日志内容、来源和时间

`log!("key": "value", ...)` 自动使用当前时间。
`log!(time = event_time; "key": "value", ...)` 接收 `std::time::SystemTime` 时间对象，
默认只保留整秒。key 和 value 可混用字符串字面量与 `String`，保留顺序和重复 key。每次发送提交一条日志。
`source` 和 `topic` 可选，默认均为空字符串。

```rust
use aliyun_log_producer::{log, SendOptions};
use std::time::{Duration, SystemTime};

writer.send(log!("message": "hello"))?;
writer.send_with_options(
    log!("level": "INFO", "message": "order created"),
    SendOptions::default().with_source("web-01").with_topic("orders"),
)?;

let event_time = SystemTime::now() - Duration::from_secs(60);
writer.send(log!(time = event_time; "message": "imported log"))?;
```

已有键值对集合时，也可以使用函数 `log(contents)` 或 `log_at(event_time, contents)`。

## 获取投递结果

需要确认日志是否投递成功时，使用 `send_with_callback`。
可以在闭包中捕获业务标识，将投递结果与业务关联：

```rust
use aliyun_log_producer::log;

let order_id = String::from("order-123");
writer.send_with_callback(log!("order_id": order_id.clone()), move |result| {
    match result {
        Ok(()) => println!("{order_id}: delivered"),
        Err(error) => eprintln!("{order_id}: {error}; request_id={:?}", error.request_id()),
    }
})?;
```

每条已接收日志的 callback 在成功或最终失败后执行一次。send 立即拒绝的日志不会触发 callback。
回调在后台串行执行，可能在 `send_with_callback` 返回前开始，执行顺序不保证与发送顺序一致。
回调应尽快返回；闭包需要满足 `Send + 'static`。
不要在回调中对同一个 Producer 调用 flush 或 close。

## 运行中 flush，退出时 close

| 操作 | 等待内容 | 之后能否继续发送 |
| --- | --- | --- |
| `flush_blocking()` / `flush().await` | 调用前已接收日志的最终投递结果，不等待回调 | 可以 |
| `close_blocking()` / `close().await` | 已接收日志的投递、回调和关闭完成 | 不可以 |

```rust
producer.flush_blocking()?;
// 此处可以继续使用 writer。
producer.close_blocking()?;
```

异步代码使用异步方法，避免阻塞调用线程：

```rust
producer.flush().await?;
// 此处可以继续使用 writer。
producer.close().await?;
```

这两个操作没有调用方等待超时参数；投递仍受 `max_attempts` 和 `delivery_timeout` 限制。
flush / close 成功只表示等待完成，单条日志的投递失败通过 callback 报告。
close 可以安全重复调用，且已经包含排空日志，无需先 flush。
应保证回调能返回，等待期间不要持有回调需要的锁。退出前显式关闭，不能用丢弃句柄代替。

## 发送到多个 Logstore

使用相同 endpoint 和凭证的目标可以共用一个 Producer：

```rust
use aliyun_log_producer::log;

let orders = producer.writer("my-project", "orders")?;
let audit = producer.writer("my-project", "audit")?;

let entry = log!("message": "order created");
orders.send(entry.clone())?;
audit.send(entry)?;
```

writer 可以 clone 并跨线程共享。关闭 Producer 或其任一 clone 会停止所有关联 writer 的发送。
不同 endpoint 或凭证使用不同 Producer。

## 处理容量不足

`send` 不会等待空余容量。`ProducerError::EnqueueFull` 和 `ProducerError::Closed`
会交回原始日志，由应用决定稍后重试、另行保存或报告拒绝。以下示例只重试一次：

```rust
use aliyun_log_producer::{log, ProducerError};
use std::time::Duration;

match writer.send(log!("message": "hello")) {
    Ok(()) => {}
    Err(ProducerError::EnqueueFull { log }) => {
        std::thread::sleep(Duration::from_millis(50));
        writer.send(log)?;
    }
    Err(error) => return Err(error),
}
```

第二次发送也可能失败，示例会将错误向上传递。已关闭的 Producer 不能重新启动。
带 callback 的发送重试时，需要重新传入 callback。
异步应用请将 `std::thread::sleep` 换成所用执行器的异步等待方法。

## 缩短攒批等待

对延迟敏感时，在创建前为配置设置 `with_linger(Duration::from_millis(100))`。
值越小，可能产生的请求越多。`Duration::ZERO` 表示不等待更多日志，send 本身仍不等待投递完成。
合批和重试的其他选项见[配置表](configuration_cn.md)。
