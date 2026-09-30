# Python 快速开始

## 1. 安装

使用 Python（CPython）3.8 或更高版本：

```sh
python -m pip install aliyun-log-producer
```

## 2. 创建、发送和关闭

准备已有的 Project、Logstore，以及有写入权限的 AccessKey。
在环境中设置 `ALIBABA_CLOUD_ACCESS_KEY_ID` 和 `ALIBABA_CLOUD_ACCESS_KEY_SECRET`。
将 endpoint、project 和 logstore 替换为实际值，保存为 `send.py`：

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
# send 可能抛出的异常见错误处理文档（errors_cn.md）。
try:
    writer.send({"message": "hello SLS"})
    now = int(time())
    writer.send({"level": "INFO", "message": "another log"}, time=now)
except ProducerError as exc:
    print("日志提交失败：", exc)

# 程序运行期间复用 Producer，在程序退出前统一关闭，等待投递完成。
producer.close()
```

执行 `python send.py`。Producer 创建后自动启动，日志在后台发送。

可能抛出的异常及处理方式见[错误处理](errors_cn.md)。

## 相关文档

- [使用示例](examples_cn.md)
- [配置参考](configuration_cn.md)
