# Python dynamic credentials

Create a class with a synchronous `get_credentials(self)` method returning `Credentials`. No SDK base class is required. Pass an instance as `credentials_provider`.

```python
from aliyun_log_producer import Credentials, Producer, ProducerConfig


class MyCredentialsProvider:
    def get_credentials(self):
        # Fetch credentials here and convert them to Credentials.
        return Credentials(
            access_key_id="access_key_id",
            access_key_secret="access_key_secret",
            security_token="sts_token",
            expires_at=2_000_000_000,  # Replace with the actual expiration (Unix seconds)
        )


producer = Producer(ProducerConfig(
    endpoint="cn-hangzhou.log.aliyuncs.com",
    credentials_provider=MyCredentialsProvider(),
))
```

The Producer calls `get_credentials()` once during construction, then automatically before expiration to refresh credentials. Returned credentials are cached by the SDK; no additional cache or refresh timer is needed.

Set the actual `expires_at` (integer Unix seconds) for temporary credentials. Omit the token when not using STS. Raise an exception if fetching fails; initial failure raises `ProducerError` from Producer construction.

## Related documents

- [Quick start](quickstart.md)
