# Rust 回调

发送日志时可以设置一个 callback。日志发送成功，或重试后最终失败时，回调会被调用。下面是一个使用示例：

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

提交失败时，发送方法直接返回错误或抛出异常，不会执行回调。成功入队后，回调在后台报告最终投递结果（包含重试结果）。

回调应保持简短，不要在回调中调用同一个 Producer 的 flush 或 close。程序退出前，在回调之外调用 `producer.close_blocking()`，等待日志和回调完成。

## 相关文档

- [快速开始](quickstart_cn.md)
- [错误处理](errors_cn.md)
