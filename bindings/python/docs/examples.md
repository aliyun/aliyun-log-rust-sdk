# Python usage examples

[简体中文](examples_cn.md) · [Quick start](quickstart.md) · [Configuration](configuration.md)

The snippets use the `config`, `producer` and `writer` from the quick start.
Run sends before leaving the producer's `with` block, or explicitly close at shutdown.

## Set contents, metadata and timestamps

Each send accepts one `dict[str, str]` or `Log`. Convert numeric values to strings
explicitly. `source` and `topic` default to empty strings; the timestamp defaults
to the current time. `Log` also supports duplicate keys.

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

`time` is Unix seconds in `0..2**32-1`; `time_ns` is the fractional nanosecond part
in `0..999999999`. Only whole seconds are stored by default; pass `time_ns` explicitly to include nanoseconds.
For a `Log`, set timestamps in its constructor, not in `send`. Sending takes a
snapshot and does not consume or mutate the dictionary or `Log`.

## Receive delivery results

A synchronous `on_delivery` callable receives `None` on success or a `DeliveryError`
on terminal failure. Use a closure or `functools.partial` to retain application context:

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

`DeliveryError` is a result object, not a raised exception. Its attributes are
`kind`, `message`, `http_status`, `error_code` and `request_id`; unavailable metadata
is `None`. Each accepted log's callback runs once. Rejected sends do not invoke it.

Callbacks run serially in the background and may begin before send returns;
order is not guaranteed. Use short, synchronous functions, not `async def`.
Do not call flush or close on the same producer from a callback. Callback exceptions
are reported through `sys.unraisablehook`; they do not retry the log or propagate from close.

## Flush during use; close at shutdown

| Operation | Waits for | Accepts later sends? |
| --- | --- | --- |
| `flush()` | Final delivery of logs accepted before the call; not their callbacks | Yes |
| `close()` | Pending delivery, callbacks and shutdown | No |

Use a `with Producer(config)` block for automatic close. If your application owns
the producer for longer, use `try/finally`:

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

These are synchronous methods with no wait-timeout parameter. Delivery is still
bounded by `max_attempts` and `delivery_timeout`. Neither method aggregates delivery
failures: check callbacks for individual outcomes. Close already drains logs and can
be called again safely. Ensure callbacks return and do not hold locks they need
while waiting. Close explicitly before exit; do not rely on garbage collection.

## Send to multiple logstores

Use one producer for destinations that share an endpoint and credentials:

```python
orders = producer.writer("my-project", "orders")
audit = producer.writer("my-project", "audit")
orders.send({"message": "order created"})
audit.send({"message": "order created"})
```

The producer and writers can be shared across threads. Closing the producer stops
sends from all its writers. Use separate producers for different endpoints or credentials.
For multiprocessing, create a new producer in each child process.

## Handle a full producer

`send` raises immediately when it cannot accept a log. Catch `EnqueueFullError`
if you want to retry later. This example retries once; a second failure propagates:

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

`ProducerClosedError` means the producer is closing or closed; it cannot be reopened.
Both errors inherit `ProducerError`. Input errors use `TypeError` / `ValueError`.
No callback is invoked for a rejected send; pass `on_delivery` again when retrying.

## Reduce batching delay

Pass `linger=0.1` to `ProducerConfig` before creation to reduce the wait for more logs
to 100 ms. Smaller values can produce more requests. `linger=0` disables that wait;
send still only confirms local acceptance. See the [configuration table](configuration.md)
for other batch and retry settings.
