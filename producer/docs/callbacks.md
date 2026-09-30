# Rust callbacks

You can set a callback when sending a log. It is called when delivery succeeds or ultimately fails after retries. Here is an example:

```rust,no_run
use aliyun_log_producer::log;
# use aliyun_log_producer::{LogstoreWriter, ProducerError};
# fn send(writer: &LogstoreWriter) -> Result<(), ProducerError> {

writer.send_with_callback(log!("message": "hello SLS"), |result| {
    match result {
        Ok(()) => println!("delivered"),
        Err(error) => println!("delivery failed: {error}"),
    }
})?;
# Ok(())
# }
```

If submission fails, the send method returns an error or raises an exception, and the callback does not run. Once accepted, the callback runs in the background with the final delivery result after retry handling.

Keep callbacks short. Do not flush or close the same Producer from a callback. Before exiting, call `producer.close_blocking()` outside the callback to wait for logs and callbacks to finish.

## Related documents

- [Quick start](quickstart.md)
- [Error handling](errors.md)
