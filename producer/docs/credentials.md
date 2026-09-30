# Rust dynamic credentials

Create a struct implementing `CredentialsProvider`. Its async `fetch_credentials(&self)` method returns `Result<Credentials, CredentialsError>`. Use `#[async_trait]` and configure it with `with_credentials_provider()`.

```rust,no_run
use aliyun_log_producer::{Credentials, CredentialsError, CredentialsProvider, ProducerConfig};
use aliyun_log_rust_sdk::async_trait;
use std::time::{Duration, SystemTime};

struct MyCredentialsProvider;

#[async_trait]
impl CredentialsProvider for MyCredentialsProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        // Fetch credentials here and convert them to Credentials, using their actual expiration.
        Ok(Credentials::new("access_key_id", "access_key_secret")?
            .with_security_token("sts_token")
            .with_expiration(SystemTime::UNIX_EPOCH + Duration::from_secs(2_000_000_000)))
    }
}

let config = ProducerConfig::default()
    .with_endpoint("cn-hangzhou.log.aliyuncs.com")
    .with_credentials_provider(MyCredentialsProvider);
```

The method is called for the first outgoing request, then for later requests near expiration. Creating the Producer does not fetch credentials. The SDK caches returned credentials automatically; no additional cache or refresh timer is needed.

Set the actual expiration (`SystemTime`) for temporary credentials. Omit the token when not using STS. Return `CredentialsError` if fetching fails.

## Related documents

- [Quick start](quickstart.md)
