# Python quick start

[简体中文](quickstart_cn.md) · [Overview](../README.md) · [Examples](examples.md) · [Configuration](configuration.md)

## 1. Install

Use ordinary CPython 3.8+ in a virtual environment. To install from this repository,
run the following from its root (requires a Rust toolchain and platform build dependencies):

```sh
python -m pip install ./bindings/python
```

## 2. Create, send and close

Use an existing Project and Logstore, with an AccessKey authorized to write to them.
Set `ALIBABA_CLOUD_ACCESS_KEY_ID` and `ALIBABA_CLOUD_ACCESS_KEY_SECRET` in your environment.
Replace the endpoint, project and logstore below, and save this as `send.py`:

```python
import os
from time import time
from aliyun_log_producer import Producer, ProducerConfig


config = ProducerConfig(
    endpoint="cn-hangzhou.log.aliyuncs.com",
    access_key_id=os.environ["ALIBABA_CLOUD_ACCESS_KEY_ID"],
    access_key_secret=os.environ["ALIBABA_CLOUD_ACCESS_KEY_SECRET"],
)
producer = Producer(config)
writer = producer.writer("my-project", "my-logstore")
writer.send({"message": "hello SLS"})
now = int(time())
writer.send({"level": "INFO", "message": "another log"}, time=now)

# Reuse the producer while the application runs; close at shutdown to wait for delivery.
producer.close()
```

Run `python send.py`. Creating the producer starts it automatically.

`send` uses the current timestamp by default; returning successfully means the log
was accepted locally. Call `close()` before exit to wait for pending delivery.
In long-running applications, reuse the producer and writer and close at shutdown.
