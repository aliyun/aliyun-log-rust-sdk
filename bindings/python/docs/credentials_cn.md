# Python 动态凭证

创建一个类，实现同步方法 `get_credentials(self)`，返回 `Credentials`。无需继承任何 SDK 类，通过 `credentials_provider` 配置即可。

```python
from aliyun_log_producer import Credentials, Producer, ProducerConfig


class MyCredentialsProvider:
    def get_credentials(self):
        # 在这里执行你的获取凭证逻辑，并转换为 Credentials。
        return Credentials(
            access_key_id="access_key_id",
            access_key_secret="access_key_secret",
            security_token="sts_token",
            expires_at=2_000_000_000,  # 替换为真实过期时间（Unix 秒）
        )


producer = Producer(ProducerConfig(
    endpoint="cn-hangzhou.log.aliyuncs.com",
    credentials_provider=MyCredentialsProvider(),
))
```

创建 Producer 时调用一次 `get_credentials()`；之后会在凭证过期前自动调用并刷新。返回的凭证由 SDK 自动缓存，无需额外缓存或定时刷新。

临时凭证应设置真实的 `expires_at`（Unix 秒，整数）。不使用 STS 时可省略 token。获取失败时直接抛出异常；首次获取失败会使 Producer 创建抛出 `ProducerError`。

## 相关文档

- [快速开始](quickstart_cn.md)
