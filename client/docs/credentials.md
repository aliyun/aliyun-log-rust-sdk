# Dynamic Credentials

Configure a credentials provider to obtain credentials without replacing the client
when temporary credentials change. The SDK manages refreshes automatically. Use the
creation functions below and pass their result to `Config::builder().credentials_provider()`.

## Available Providers

ECS RAM Role is currently the only built-in dynamic provider. Environment credentials,
static credentials, and custom providers are also supported.

| Provider     | Creation function                                                               | Use case                                                         |
| ------------ | ------------------------------------------------------------------------------- | ---------------------------------------------------------------- |
| ECS RAM Role | `ecs_ram_role_credentials_provider(role_name)`                                  | Temporary credentials for a role attached to an ECS instance     |
| Environment  | `environment_credentials_provider()`                                            | Read a fixed credentials snapshot from environment variables     |
| Static       | `static_credentials_provider(access_key_id, access_key_secret, security_token)` | A fixed set of credentials; does not renew temporary credentials |
| Custom       | Implement `CredentialsProvider` and expose your own creation function           | A credentials source managed by your application                 |

## ECS RAM Role

### Prerequisites

1. Create a RAM role trusted by ECS and grant it the SLS permissions your application needs.
2. Attach that role to the ECS instance running your application.
3. Ensure that the application can access the instance metadata service and that the
   instance allows normal, tokenless metadata access (IMDSv1). IMDSv2-only instances
   are not supported by this provider.

For role creation, permissions, attachment, and metadata access modes, see
[Attach a RAM role to an ECS instance](https://www.alibabacloud.com/help/en/ecs/user-guide/attach-an-instance-ram-role-to-an-ecs-instance).

### Create a Client

Pass the **role name**, not its ARN. The name is required; the provider does not
automatically choose a role. Creating the provider and client does not fetch
credentials. Credentials are obtained when the client sends a request.

```rust
use aliyun_log_rust_sdk::{ecs_ram_role_credentials_provider, Client, Config, FromConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ecs_ram_role_credentials_provider("my-ecs-role")?;
    let config = Config::builder()
        .endpoint("cn-hangzhou.log.aliyuncs.com")
        .credentials_provider(provider)
        .build()?;
    let client = Client::from_config(config)?;

    // Use client.put_logs(...), client.get_logs(...), and other APIs as usual.
    // Await requests in a Tokio runtime, as shown in the Client README.
    Ok(())
}
```

The provider supplies the role's AccessKey ID, AccessKey secret, STS token,
expiration, and update time. You do not need to configure access keys separately.
The SDK only checks that the role name is nonempty.

### Configure the Fetch Timeout

Each credentials fetch attempt defaults to a 5-second timeout. Set a nonzero
duration with `credentials_fetch_timeout`; it is separate from the SLS HTTP
request timeout.

```rust
use aliyun_log_rust_sdk::{ecs_ram_role_credentials_provider, Config};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::builder()
        .endpoint("cn-hangzhou.log.aliyuncs.com")
        .credentials_provider(ecs_ram_role_credentials_provider("my-ecs-role")?)
        .credentials_fetch_timeout(Duration::from_secs(3))
        .request_timeout(Duration::from_secs(60))
        .build()?;
    Ok(())
}
```

## Environment Credentials

The default helper immediately reads and validates these variables:

| Variable                          | Requirement                               |
| --------------------------------- | ----------------------------------------- |
| `ALIBABA_CLOUD_ACCESS_KEY_ID`     | Required and nonempty                     |
| `ALIBABA_CLOUD_ACCESS_KEY_SECRET` | Required and nonempty                     |
| `ALIBABA_CLOUD_SECURITY_TOKEN`    | Optional; missing or empty means no token |

Set the variables before creating the provider:

```no_run
use aliyun_log_rust_sdk::{environment_credentials_provider, Client, Config, FromConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::builder()
        .endpoint("cn-hangzhou.log.aliyuncs.com")
        .credentials_provider(environment_credentials_provider()?)
        .build()?;
    let client = Client::from_config(config)?;
    Ok(())
}
```

To change individual names, use the builder helper. Here the STS token keeps its
default variable name; it is fine if that variable does not exist:

```no_run
use aliyun_log_rust_sdk::environment_credentials_provider_builder;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = environment_credentials_provider_builder()
        .with_access_key_id_env("MY_ACCESS_KEY_ID")
        .with_access_key_secret_env("MY_ACCESS_KEY_SECRET")
        .build()?;
    Ok(())
}
```

Use `.with_security_token_env("MY_SECURITY_TOKEN")` to override the token variable
if needed. The builder reads and validates values when `.build()` creates the
provider. Missing required variables, empty access keys, and non-Unicode values
return an error immediately; a missing or empty token is valid by design.
Values are not trimmed.

The provider captures a snapshot with no expiration or update time. Subsequent
environment changes do not affect it or its clones. STS credentials can still
expire at the service; this provider does not renew them.

## Static Credentials

Use the static creation function when you already have a fixed set of credentials:

```rust
use aliyun_log_rust_sdk::{static_credentials_provider, Client, Config, FromConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = static_credentials_provider("access_key_id", "access_key_secret", None)?;
    // For a fixed STS credential, pass Some("sts_token".to_string()) instead of None.
    let config = Config::builder()
        .endpoint("cn-hangzhou.log.aliyuncs.com")
        .credentials_provider(provider)
        .build()?;
    let client = Client::from_config(config)?;
    Ok(())
}
```

The static helper validates the keys and does not set an expiration. It cannot
renew an STS credential when the service expires it. Use ECS RAM Role or a custom
provider for automatic renewal. The existing `.access_key()` and `.sts()` APIs
remain supported.

## Custom Providers

Implement the async `CredentialsProvider::fetch_credentials()` method returning `Result<Credentials, CredentialsError>`, then configure it with `.credentials_provider()`.

```rust,no_run
use aliyun_log_rust_sdk::{async_trait, Config, Credentials, CredentialsError, CredentialsProvider};
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

let config = Config::builder()
    .endpoint("cn-hangzhou.log.aliyuncs.com")
    .credentials_provider(MyCredentialsProvider)
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

The method is called for the first outgoing request, then for later requests near expiration. Creating the Client does not fetch credentials. The SDK caches returned credentials automatically; no additional cache or refresh timer is needed.

Set the actual expiration (`SystemTime`) for temporary credentials. Omit the token when not using STS. Return `CredentialsError` if fetching fails.

## Error Handling and Configuration

- If fetching credentials fails, the SDK uses previously obtained credentials,
  **even if they have expired**. The cloud service can still reject such credentials.
- If no credentials have been obtained, the request returns `Error::Credentials`.
  A timeout, invalid credentials response, or temporarily suppressed fetch may be
  the underlying cause.
- Do not combine `.credentials_provider()` with `.access_key()` or `.sts()` in the
  same configuration. `build()` returns a configuration error for conflicting sources.
- The endpoint and the SLS permissions of the role must match the resources your
  application accesses. Successfully obtaining credentials does not grant additional permissions.
