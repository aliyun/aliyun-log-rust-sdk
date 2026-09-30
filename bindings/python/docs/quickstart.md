# Python quick start

## 1. Install

Use Python (CPython) 3.8 or later:

```sh
python -m pip install aliyun-log-producer
```

## 2. Create, send and close

Use an existing Project and Logstore, with an AccessKey authorized to write to them.
Set `ALIBABA_CLOUD_ACCESS_KEY_ID` and `ALIBABA_CLOUD_ACCESS_KEY_SECRET` in your environment.
Replace the endpoint, project and logstore below, and save this as `send.py`:

```python
import os
from time import time
from aliyun_log_producer import Producer, ProducerConfig, ProducerError


config = ProducerConfig(
    endpoint="cn-hangzhou.log.aliyuncs.com",
    access_key_id=os.environ["ALIBABA_CLOUD_ACCESS_KEY_ID"],
    access_key_secret=os.environ["ALIBABA_CLOUD_ACCESS_KEY_SECRET"],
)
producer = Producer(config)
writer = producer.writer("my-project", "my-logstore")
# Possible send exceptions: see the Error handling guide (errors.md).
try:
    writer.send({"message": "hello SLS"})
    now = int(time())
    writer.send({"level": "INFO", "message": "another log"}, time=now)
except ProducerError as exc:
    print("Log submission failed:", exc)

# Reuse the producer while the application runs; close at shutdown to wait for delivery.
producer.close()
```

Run `python send.py`. The Producer starts automatically and sends logs in the background.

See [Error handling](errors.md) for possible exceptions and how to handle them.

## Related documents

- [Examples](examples.md)
- [Configuration](configuration.md)
