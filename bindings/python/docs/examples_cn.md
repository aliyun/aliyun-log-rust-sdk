# Python 使用示例

[English](examples.md) · [快速开始](quickstart_cn.md) · [配置参考](configuration_cn.md)

以下片段沿用快速开始中的 `config`、`producer` 和 `writer`。
发送应在 Producer 的 `with` 块退出前执行，或由应用在退出时显式关闭。

## 设置日志内容、来源和时间

每次发送一条 `dict[str, str]` 或 `Log`。数字等值需显式转换为字符串。
`source` 和 `topic` 默认为空字符串，时间默认取当前时间。`Log` 还支持重复的 key。

```python
from aliyun_log_producer import Log

writer.send(
    {"level": "INFO", "message": "order created", "order_id": str(123)},
    source="web-01",
    topic="orders",
)
writer.send({"message": "imported log"}, time=1700000000, time_ns=123456789)

log = Log([("message", "imported log"), ("tag", "a"), ("tag", "b")], time=1700000000)
writer.send(log)
```

`time` 为 Unix 秒，范围为 `0..2**32-1`；`time_ns` 为秒内纳秒部分，范围为 `0..999999999`。
默认只保存整秒；只有显式设置 `time_ns` 才包含纳秒部分。
使用 `Log` 时应在构造时设置时间，不要再向 `send` 传时间参数。
发送会取日志快照，不消耗或修改传入的字典或 `Log`。

## 获取投递结果

同步 `on_delivery` 函数在成功时收到 `None`，最终失败时收到 `DeliveryError`。
可以使用闭包或 `functools.partial` 保留业务上下文：

```python
from functools import partial


def on_delivery(order_id, error):
    if error is None:
        print(f"{order_id}: delivered")
    else:
        print(f"{order_id}: {error.kind}: {error.message}; request_id={error.request_id}")


writer.send(
    {"order_id": "order-123"},
    on_delivery=partial(on_delivery, "order-123"),
)
```

`DeliveryError` 是结果对象，不是抛出的异常。它提供 `kind`、`message`、`http_status`、
`error_code` 和 `request_id`，不存在的元数据为 `None`。
每条已接收日志的 callback 执行一次；send 立即拒绝的日志不会触发 callback。

回调在后台串行执行，可能在 send 返回前开始，不保证执行顺序。
请使用能尽快返回的同步函数，不要使用 `async def`，也不要在回调中对同一个 Producer 调用 flush 或 close。
回调异常通过 `sys.unraisablehook` 报告，不会触发日志重试，也不会从 close 抛出。

## 运行中 flush，退出时 close

| 操作 | 等待内容 | 之后能否继续发送 |
| --- | --- | --- |
| `flush()` | 调用前已接收日志的最终投递结果，不等待回调 | 可以 |
| `close()` | 已接收日志的投递、回调和关闭完成 | 不可以 |

使用 `with Producer(config)` 可以自动关闭。如果应用需要长期持有 Producer，可以使用 `try/finally`：

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

这两个方法都是同步方法，没有等待超时参数；投递仍受 `max_attempts` 和 `delivery_timeout` 限制。
它们不汇总投递失败，单条日志的结果请查看 callback。close 已经包含排空日志，可以安全重复调用。
应保证回调能返回，等待期间不要持有回调需要的锁。退出前显式关闭，不要依赖垃圾回收。

## 发送到多个 Logstore

使用相同 endpoint 和凭证的目标可以共用一个 Producer：

```python
orders = producer.writer("my-project", "orders")
audit = producer.writer("my-project", "audit")
orders.send({"message": "order created"})
audit.send({"message": "order created"})
```

Producer 和 writer 可以跨线程共享。关闭 Producer 会停止所有关联 writer 的发送。
不同 endpoint 或凭证使用不同 Producer。多进程应用应在每个子进程中创建新的 Producer。

## 处理容量不足

`send` 无法接收日志时会立即抛出异常。需要稍后重试时，捕获 `EnqueueFullError`。
以下示例只重试一次，第二次失败会继续抛出：

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

`ProducerClosedError` 表示 Producer 正在关闭或已关闭，不能重新启动。
这两个异常都继承 `ProducerError`；输入错误使用 `TypeError` / `ValueError`。
被拒绝的发送不触发 callback，重试时需重新传入 `on_delivery`。

## 缩短攒批等待

在创建前向 `ProducerConfig` 传入 `linger=0.1`，将等待更多日志的时间缩短为 100 ms。
值越小，可能产生的请求越多。`linger=0` 表示不等待更多日志，send 仍然只确认本地接收。
合批和重试的其他选项见[配置表](configuration_cn.md)。
