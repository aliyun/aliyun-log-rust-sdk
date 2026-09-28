# Python 使用示例

[English](examples.md) · [快速开始](quickstart_cn.md) · [配置参考](configuration_cn.md) · [错误处理](errors_cn.md)

以下示例使用[快速开始](quickstart_cn.md)中创建的 Producer 和 writer。

## 标记日志来源和主题

用 `source` 标记日志来自哪台机器，用 `topic` 区分业务，例如订单日志：

```python
writer.send(
    {"level": "INFO", "message": "order created", "order_id": str(123)},
    source="web-01",
    topic="orders",
)
```

## 设置日志时间

默认使用当前时间。可通过 `time` 指定 Unix 秒，通过 `time_ns` 补充纳秒部分：

```python
writer.send({"message": "imported log"}, time=1700000000)
writer.send({"message": "precise time"}, time=1700000000, time_ns=123456789)
```

## 确认日志是否发送成功

`send` 返回时，日志可能还没到达 SLS。设置 `on_delivery`，可以收到发送成功或失败的通知：

```python
def on_delivery(error):
    if error is None:
        print("delivered")
    else:
        print(f"failed: {error.message}; request_id={error.request_id}")


writer.send({"message": "hello"}, on_delivery=on_delivery)
```

如果 `send` 直接抛出异常，日志没有被接收，也不会触发回调。
回调应尽快返回。不要在回调中关闭或 flush 同一个 Producer。

## 等待发送完成与关闭

`flush()` 可能阻塞当前线程，仅在需要等待之前的日志发送结束时使用，无需每次发送后调用。

| 需要做什么 | 调用方法 |
| --- | --- |
| 等之前的日志发送结束，然后继续发送 | `flush()`，不等待回调 |
| 程序退出前，等日志发送和回调结束 | `close()`，之后不能再发送 |

用 `try/finally` 确保退出前关闭 Producer：

```python
producer = Producer(config)
try:
    writer = producer.writer("my-project", "my-logstore")
    writer.send({"message": "first"})
    producer.flush()
    writer.send({"message": "second"})
finally:
    producer.close()
```

也可以使用 `with Producer(config)`，离开代码块时自动关闭。
关闭前无需再调用 `flush()`。这两个方法都不会报告单条日志的发送失败；需要确认结果时，请使用回调。

## 发送到多个 Logstore

使用同一个 SLS 服务地址和凭证时，可以共用一个 Producer：

```python
orders = producer.writer("my-project", "orders")
audit = producer.writer("my-project", "audit")
orders.send({"message": "order created"})
audit.send({"message": "order created"})
```

Producer 和 writer 可以在线程之间共用。如果使用多进程，请在每个子进程中分别创建 Producer。

## 发送太快，队列满了怎么办

收到 `EnqueueFullError` 时，可以稍后重试。下面的示例等待 50 ms 后重试一次：

```python
import time
from aliyun_log_producer import EnqueueFullError

log = {"message": "hello"}
try:
    writer.send(log)
except EnqueueFullError:
    time.sleep(0.05)
    writer.send(log)
```

第二次发送仍可能失败，需要由应用处理。使用回调时，重试也要传入 `on_delivery`。

## 让日志更快发出

设置 `linger=0.1`，将等待更多日志的时间缩短为 100 ms；设为 `0` 则不等待。
等待越短，请求次数可能越多。其他选项见[配置表](configuration_cn.md)。
