# 动态凭证

[English](credentials.md) | 简体中文 | [Client README](../README_CN.md)

配置 credentials provider 后，临时凭证发生变化时无需重新创建客户端，SDK 会自动
管理凭证刷新。使用下面的便捷创建函数，将返回的 provider 传给
`Config::builder().credentials_provider()` 即可。

## 可用 Provider

目前内置的动态 provider 只有 ECS RAM Role，同时支持环境变量凭证、静态凭证和自定义 provider。

| Provider | 创建函数 | 适用场景 |
| --- | --- | --- |
| ECS RAM Role | `ecs_ram_role_credentials_provider(role_name)` | 获取 ECS 实例绑定角色的临时凭证 |
| 环境变量 | `environment_credentials_provider()` | 从环境变量读取固定凭证快照 |
| 静态凭证 | `static_credentials_provider(access_key_id, access_key_secret, security_token)` | 使用固定凭证，不会续期临时凭证 |
| 自定义 | 实现 `CredentialsProvider`，提供自己的创建函数 | 接入业务管理的凭证来源 |

## ECS RAM Role

### 前提条件

1. 创建信任 ECS 服务的 RAM 角色，并授予业务需要的 SLS 权限。
2. 将角色绑定到运行应用的 ECS 实例。
3. 确保应用能够访问实例元数据服务，且实例允许普通模式、不携带令牌的元数据访问
   （IMDSv1）。该 provider 不支持强制使用 IMDSv2 的实例。

角色创建、权限配置、角色绑定和元数据访问模式的操作说明，请参见官方文档
[给 ECS 实例授予 RAM 角色](https://www.alibabacloud.com/help/zh/ecs/user-guide/attach-an-instance-ram-role-to-an-ecs-instance)。

### 创建客户端

传入**角色名称**，不是角色 ARN。名称必填，provider 不会自动选择角色。
创建 provider 和客户端时不会获取凭证，客户端发送请求时才会获取。

```rust
use aliyun_log_rust_sdk::{ecs_ram_role_credentials_provider, Client, Config, FromConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = ecs_ram_role_credentials_provider("my-ecs-role")?;
    let config = Config::builder()
        .endpoint("cn-hangzhou.log.aliyuncs.com")
        .credentials_provider(provider)
        .build()?;
    let client = Client::from_config(config)?;

    // 之后照常使用 client.put_logs(...)、client.get_logs(...) 等 API。
    // 在 Tokio 运行时中 await 请求，完整调用方式见 Client README。
    Ok(())
}
```

Provider 会提供角色的 AccessKey ID、AccessKey Secret、STS Token、过期时间和
更新时间，无需另外配置 AK。SDK 仅检查角色名称非空。

### 配置凭证获取超时

每次凭证获取默认超时 5 秒。可通过 `credentials_fetch_timeout` 设置非零时长，
该配置与 SLS HTTP 请求超时相互独立。

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

## 环境变量凭证

无参 helper 会立即读取并校验以下环境变量：

| 环境变量 | 要求 |
| --- | --- |
| `ALIBABA_CLOUD_ACCESS_KEY_ID` | 必须存在且非空 |
| `ALIBABA_CLOUD_ACCESS_KEY_SECRET` | 必须存在且非空 |
| `ALIBABA_CLOUD_SECURITY_TOKEN` | 可选，不存在或为空时按无 token 处理 |

在创建 provider 前设置环境变量：

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

需要修改某个变量名时，使用 builder helper。下面只修改 AK 两个变量名，STS 仍使用
默认变量名，该变量不存在也不会报错：

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

如需修改 token 变量名，添加 `.with_security_token_env("MY_SECURITY_TOKEN")`。
Builder 在 `.build()` 创建 provider 时读取并校验值。必填变量缺失、AK 为空或值不是
合法 Unicode 时立即返回错误；STS Token 不存在或为空是正常情况，不报错。
变量值保留原样，不会去除首尾空白。

Provider 保存创建时的凭证快照，不设置过期时间和更新时间。后续环境变量变化不会
影响该 provider 及其克隆。STS 凭证仍可能在服务端过期，该 provider 不会为其续期。

## 静态凭证

已有固定凭证时，使用静态 provider 的创建函数：

```rust
use aliyun_log_rust_sdk::{static_credentials_provider, Client, Config, FromConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let provider = static_credentials_provider("access_key_id", "access_key_secret", None)?;
    // 使用固定 STS 凭证时，将 None 换成 Some("sts_token".to_string())。
    let config = Config::builder()
        .endpoint("cn-hangzhou.log.aliyuncs.com")
        .credentials_provider(provider)
        .build()?;
    let client = Client::from_config(config)?;
    Ok(())
}
```

静态 helper 会校验 AK，但不设置过期时间，也不能在服务端判定 STS 凭证过期后为其
续期。需要自动续期时，应使用 ECS RAM Role 或自定义 provider。
原有 `.access_key()` 和 `.sts()` 接口继续支持。

## 自定义 Provider

使用 SDK 导出的 `async_trait` 属性实现公开的 `CredentialsProvider` trait，
其中 `fetch_credentials()` 返回 `Result<Credentials, CredentialsError>`。
为自己的 provider 提供创建函数，再像内置 helper 一样将结果传给
`.credentials_provider()`。API 约定请参见
[trait 文档](https://docs.rs/aliyun-log-rust-sdk/latest/aliyun_log_rust_sdk/trait.CredentialsProvider.html)。
下面的示例调用业务管理的 HTTPS 凭证服务，响应 JSON 包含 `SourceCredentials`
所示字段，其中过期时间为 RFC 3339 字符串。请按自己的服务调整请求与响应结构。
示例使用 `reqwest`、启用 `derive` 的 `serde`、`serde_json` 和 `chrono`。

```rust
use aliyun_log_rust_sdk::{
    async_trait, Config, Credentials, CredentialsError, CredentialsProvider,
};
use serde::Deserialize;

struct HttpCredentialsProvider {
    endpoint: String,
    client: reqwest::Client,
}

// JSON returned by your application's credentials service.
#[derive(Deserialize)]
struct SourceCredentials {
    access_key_id: String,
    access_key_secret: String,
    security_token: Option<String>,
    expiration: String,
}

#[async_trait]
impl CredentialsProvider for HttpCredentialsProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        let body = self.client.get(&self.endpoint).send().await
            .map_err(CredentialsError::provider)?
            .error_for_status().map_err(CredentialsError::provider)?
            .bytes().await.map_err(CredentialsError::provider)?;
        let source: SourceCredentials = serde_json::from_slice(&body)
            .map_err(CredentialsError::provider)?;
        let expiration = chrono::DateTime::parse_from_rfc3339(&source.expiration)
            .map_err(CredentialsError::provider)?;
        let mut credentials = Credentials::new(source.access_key_id, source.access_key_secret)?
            .with_expiration(expiration.into());
        if let Some(token) = source.security_token {
            credentials = credentials.with_security_token(token);
        }
        Ok(credentials)
    }
}

fn http_credentials_provider(endpoint: impl Into<String>) -> impl CredentialsProvider {
    HttpCredentialsProvider { endpoint: endpoint.into(), client: reqwest::Client::new() }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::builder()
        .endpoint("cn-hangzhou.log.aliyuncs.com")
        .credentials_provider(http_credentials_provider("https://credentials.example.com/current"))
        .build()?;
    Ok(())
}
```

通过 `Credentials::new(access_key_id, access_key_secret)` 构造返回的凭证，
然后按需设置可选字段：

| 字段 | API | 含义 |
| --- | --- | --- |
| AccessKey ID 和 Secret | `Credentials::new(id, secret)` | 必填，均不能为空 |
| STS Token | `.with_security_token(token)` | 可选，空字符串按缺失处理 |
| 过期时间 | `.with_expiration(SystemTime)` | 凭证来源给出的实际过期时间，仅无限期凭证才省略 |
| 更新时间 | `.with_update_time(SystemTime)` | 可选元数据，不影响有效期 |

应从凭证来源获取新凭证，并返回实际过期时间，不要在本地延长已有临时凭证的有效期。
来源错误可通过 `CredentialsError::provider(error)` 转换。
Provider 必须支持并发调用，并使用支持取消的异步 I/O。

自定义 provider 无需实现 `Clone`；需要共享句柄时，可以使用 `Arc<YourProvider>`、
`Arc<dyn CredentialsProvider>` 或 `SharedCredentialsProvider`。
内置 ECS、环境变量和静态 provider 已支持 `Clone`。

## 错误处理与配置约束

- 获取失败时，SDK 会使用之前取得的凭证，**即使它已经过期**。云服务仍可能拒绝过期凭证。
- 如果从未取得凭证，请求返回 `Error::Credentials`。具体原因可能是超时、凭证响应无效，
  或凭证获取暂时受到抑制。
- 同一配置中不要混用 `.credentials_provider()` 与 `.access_key()`、`.sts()`；
  凭证来源冲突时，`build()` 返回配置错误。
- Endpoint 和角色的 SLS 权限应匹配应用访问的资源。成功获取凭证并不意味着拥有额外权限。
