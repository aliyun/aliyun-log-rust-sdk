# Python callbacks

You can set a callback when sending a log. It is called when delivery succeeds or ultimately fails after retries. Here is an example:

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

If submission fails, the send method returns an error or raises an exception, and the callback does not run. Once accepted, the callback runs in the background with the final delivery result after retry handling.

Keep callbacks short. Do not flush or close the same Producer from a callback. Before exiting, call `producer.close()` outside the callback to wait for logs and callbacks to finish.

## Related documents

- [Quick start](quickstart.md)
- [Error handling](errors.md)
