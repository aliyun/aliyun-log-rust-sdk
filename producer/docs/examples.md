# Rust usage examples

[简体中文](examples_cn.md) · [Quick start](quickstart.md) · [Configuration](configuration.md) · [Error handling](errors.md)

These examples use the Producer and writer created in the [quick start](quickstart.md).

## Choose a send method

| What you need | Method |
| --- | --- |
| Send a log | `send(log)` |
| Set the source and topic | `send_with_options(log, options)` |
| Receive the delivery result | `send_with_callback(log, callback)` |
| Set the source, topic, and callback | `send_with_options_and_callback(log, options, callback)` |

## Label the source and topic

Use `source` to identify the machine and `topic` to group logs by purpose, such as orders:

```rust
use aliyun_log_producer::{log, SendOptions};

writer.send_with_options(
    log!("level": "INFO", "message": "order created"),
    SendOptions::default().with_source("web-01").with_topic("orders"),
)?;
```

## Set the log time

`log!("key": "value", ...)` uses the current time.
Use `log!(time = event_time; "key": "value", ...)` to set the log time.

```rust
use aliyun_log_producer::log;
use std::time::{Duration, SystemTime};

let event_time = SystemTime::now() - Duration::from_secs(60);
writer.send(log!(time = event_time; "message": "imported log"))?;
```

## Check whether logs were delivered

A log may not have reached SLS when `send` returns. Use `send_with_callback` to receive the delivery result:

```rust
use aliyun_log_producer::log;

writer.send_with_callback(log!("message": "hello"), |result| {
    match result {
        Ok(()) => println!("delivered"),
        Err(error) => eprintln!("failed: {error}; request_id={:?}", error.request_id()),
    }
})?;
```

If the send method returns an error, the log was not accepted and its callback will not run.
Keep callbacks short. Do not close or flush the same Producer from a callback.

## Wait for logs and close the Producer

`flush_blocking()` may block the current thread. Use it only when you need to wait for earlier logs to finish sending, not after every send.

| What you need | Method |
| --- | --- |
| Wait for earlier logs to finish sending, then keep sending | `flush_blocking()`; does not wait for callbacks |
| Wait for logs and callbacks before exiting | `close_blocking()`; stops further sends |

```rust
producer.flush_blocking()?;
// Continue using the writer here.
producer.close_blocking()?;
```

In async functions, use `flush().await` and `close().await` to wait without blocking the current thread:

```rust
producer.flush().await?;
// Continue using the writer here.
producer.close().await?;
```

There is no need to flush before closing. Neither operation reports individual delivery failures; use a callback to check results.

## Send to multiple logstores

Logstores with the same SLS endpoint and credentials can share a Producer:

```rust
use aliyun_log_producer::log;

let orders = producer.writer("my-project", "orders")?;
let audit = producer.writer("my-project", "audit")?;

let entry = log!("message": "order created");
orders.send(entry.clone())?;
audit.send(entry)?;
```

Threads can use clones of the same writer. You do not need a separate Producer for each thread.

## Handle a full queue

If you receive `ProducerError::EnqueueFull`, you can retrieve the log and retry later. This example waits 50 ms and retries once:

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

The second send can still fail and needs to be handled by your application. If you use a callback, pass it again when retrying.
In async code, use an async wait such as `tokio::time::sleep`.

## Send logs sooner

Set `with_linger(Duration::from_millis(100))` to wait up to 100 ms for more logs, or `Duration::ZERO` to skip the wait.
Shorter waits can mean more requests. See the [configuration table](configuration.md) for other options.
