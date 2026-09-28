# Python usage examples

[简体中文](examples_cn.md) · [Quick start](quickstart.md) · [Configuration](configuration.md)

These examples use the Producer and writer created in the [quick start](quickstart.md).

## Label the source and topic

Use `source` to identify the machine and `topic` to group logs by purpose, such as orders:

```python
writer.send(
    {"level": "INFO", "message": "order created", "order_id": str(123)},
    source="web-01",
    topic="orders",
)
```

## Set the log time

Logs use the current time by default. Set `time` in Unix seconds and optionally add `time_ns` for the nanosecond part:

```python
writer.send({"message": "imported log"}, time=1700000000)
writer.send({"message": "precise time"}, time=1700000000, time_ns=123456789)
```

## Check whether logs were delivered

A log may not have reached SLS when `send` returns. Set `on_delivery` to receive the delivery result:

```python
def on_delivery(error):
    if error is None:
        print("delivered")
    else:
        print(f"failed: {error.message}; request_id={error.request_id}")


writer.send({"message": "hello"}, on_delivery=on_delivery)
```

If `send` raises an exception, the log was not accepted and its callback will not run.
Keep callbacks short. Do not close or flush the same Producer from a callback.

## Wait for logs and close the Producer

`flush()` may block the current thread. Use it only when you need to wait for earlier logs to finish sending, not after every send.

| What you need | Method |
| --- | --- |
| Wait for earlier logs to finish sending, then keep sending | `flush()`; does not wait for callbacks |
| Wait for logs and callbacks before exiting | `close()`; stops further sends |

Use `try/finally` to close the Producer before exiting:

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

You can also use `with Producer(config)` to close automatically when leaving the block.
There is no need to flush before closing. Neither method reports individual delivery failures; use a callback to check results.

## Send to multiple logstores

Logstores with the same SLS endpoint and credentials can share a Producer:

```python
orders = producer.writer("my-project", "orders")
audit = producer.writer("my-project", "audit")
orders.send({"message": "order created"})
audit.send({"message": "order created"})
```

You can share the Producer and writers across threads. For multiprocessing, create a Producer in each child process.

## Handle a full queue

If you receive `EnqueueFullError`, you can retry later. This example waits 50 ms and retries once:

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

The second send can still fail and needs to be handled by your application. If you use a callback, pass `on_delivery` again when retrying.
If you receive `ProducerClosedError`, check whether the Producer was closed too early.

## Send logs sooner

Set `linger=0.1` to wait up to 100 ms for more logs, or `0` to skip the wait.
Shorter waits can mean more requests. See the [configuration table](configuration.md) for other options.
