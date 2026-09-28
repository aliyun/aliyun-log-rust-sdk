# Rust usage examples

[简体中文](examples_cn.md) · [Quick start](quickstart.md) · [Configuration](configuration.md)

These snippets use the `producer` and `writer` created in the quick start.
Put fallible snippets in a function returning `Result<(), ProducerError>` and close
the producer when the application shuts down.

## Four ways to send

| Method | source / topic | Delivery callback |
| --- | --- | --- |
| `send(log)` | Empty defaults | No |
| `send_with_options(log, options)` | Custom | No |
| `send_with_callback(log, callback)` | Empty defaults | Yes |
| `send_with_options_and_callback(log, options, callback)` | Custom | Yes |

All four methods accept the log or return an error immediately, without waiting for delivery.

## Send logs with metadata or an explicit timestamp

`log!("key": "value", ...)` uses the current timestamp.
`log!(time = event_time; "key": "value", ...)` accepts a `std::time::SystemTime`
and keeps only whole seconds. Keys and values accept string literals or owned strings; order and duplicate keys are preserved. Each send submits one log. `source` and `topic` are optional and default to empty strings.

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

For an existing collection of pairs, use the functions `log(contents)` or `log_at(event_time, contents)`.

## Receive delivery results

Use `send_with_callback` when you need to know whether a log was delivered.
Capture an identifier in the closure to associate the result with your application.

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

Each accepted log's callback runs once after success or terminal failure. A send
rejected immediately does not invoke the callback. Callbacks execute serially in
the background and may run before `send_with_callback` returns; their order is not
guaranteed. Keep callbacks short. Closures must be `Send + 'static`.
Do not call flush or close on this producer from its callback.

## Flush during use; close at shutdown

| Operation | Waits for | Accepts later sends? |
| --- | --- | --- |
| `flush_blocking()` / `flush().await` | Final delivery of logs accepted before the call; not their callbacks | Yes |
| `close_blocking()` / `close().await` | Pending delivery, callbacks and shutdown | No |

```rust
producer.flush_blocking()?;
// Continue using the writer here.
producer.close_blocking()?;
```

In async code, use the async methods instead of blocking the calling thread:

```rust
producer.flush().await?;
// Continue using the writer here.
producer.close().await?;
```

Both operations wait without a caller-specified timeout. Delivery is still bounded
by `max_attempts` and `delivery_timeout`. Successful flush/close means waiting completed;
individual delivery failures are reported through callbacks. Close can be called again
safely, and includes flushing. Ensure callbacks return, and do not hold locks they need
while waiting. Explicitly close before process exit; dropping handles is not a substitute.

## Send to multiple logstores

Use one producer for destinations that share an endpoint and credentials:

```rust
use aliyun_log_producer::log;

let orders = producer.writer("my-project", "orders")?;
let audit = producer.writer("my-project", "audit")?;

let entry = log!("message": "order created");
orders.send(entry.clone())?;
audit.send(entry)?;
```

Writers can be cloned and shared across threads. Closing the producer (or any clone)
stops sends from all its writers. Use separate producers for different endpoints or credentials.

## Handle a full producer

`send` does not wait for capacity. `ProducerError::EnqueueFull` and
`ProducerError::Closed` return ownership of the original log. The application decides
whether to wait, save it elsewhere or report the rejection. For example, retry once:

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

The second send can also fail; this example propagates that error. A closed producer
cannot be reopened. When using callbacks, pass a new callback for a retried send.
For async applications, use your executor's delay instead of `std::thread::sleep`.

## Reduce batching delay

For latency-sensitive logs, set `with_linger(Duration::from_millis(100))` on the
configuration before creation. Smaller values can produce more requests. `Duration::ZERO`
disables the wait for more logs; it does not make send wait for delivery.
See the [configuration table](configuration.md) for batch and retry settings.
