# Python 回调

发送日志时可以设置一个 callback。日志发送成功，或重试后最终失败时，回调会被调用。下面是一个使用示例：

```python
from aliyun_log_producer import ProducerError


def on_delivery(error):
    if error is None:
        print("delivered")
    else:
        print(f"delivery failed: {error.message}")


try:
    writer.send({"message": "hello SLS"}, on_delivery=on_delivery)
except ProducerError as exc:
    print(f"submission failed: {exc}")
```

提交失败时，发送方法直接返回错误或抛出异常，不会执行回调。成功入队后，回调在后台报告最终投递结果（包含重试结果）。

回调应保持简短，不要在回调中调用同一个 Producer 的 flush 或 close。程序退出前，在回调之外调用 `producer.close()`，等待日志和回调完成。

## 相关文档

- [快速开始](quickstart_cn.md)
- [错误处理](errors_cn.md)
