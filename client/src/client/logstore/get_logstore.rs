use super::*;
use crate::ResponseResult;
use getset::Getters;
use serde::Deserialize;

impl crate::client::Client {
    /// Get logstore details.
    ///
    /// This method retrieves detailed information about a logstore, including its configuration,
    /// shard count, and storage settings.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore_name` - The name of the logstore to get
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// let response = client.get_logstore("my-project", "my-logstore")
    ///     .send()
    ///     .await?;
    /// println!("TTL: {} days", response.get_body().ttl());
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_logstore(
        &self,
        project: impl AsRef<str>,
        logstore_name: impl AsRef<str>,
    ) -> GetLogstoreRequestBuilder {
        GetLogstoreRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}", logstore_name.as_ref()),
            handle: self.handle.clone(),
        }
    }
}

pub struct GetLogstoreRequestBuilder {
    handle: HandleRef,
    project: String,
    path: String,
}

impl GetLogstoreRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<GetLogstoreResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<GetLogstoreRequest> {
        Ok((
            self.handle,
            GetLogstoreRequest {
                project: self.project,
                path: self.path,
            },
        ))
    }
}

struct GetLogstoreRequest {
    project: String,
    path: String,
}

impl Request for GetLogstoreRequest {
    type ResponseBody = GetLogstoreResponse;
    const HTTP_METHOD: http::Method = http::Method::GET;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }
}

/// Logstore information
#[derive(Debug, Getters, Deserialize)]
#[getset(get = "pub")]
pub struct GetLogstoreResponse {
    /// Logstore name
    #[serde(rename = "logstoreName")]
    logstore_name: String,

    /// Data retention time in days
    ttl: i32,

    /// Hot storage TTL in days
    #[serde(rename = "hot_ttl", skip_serializing_if = "Option::is_none")]
    hot_ttl: Option<i32>,

    /// Infrequent access TTL in days
    #[serde(
        rename = "infrequentAccessTTL",
        skip_serializing_if = "Option::is_none"
    )]
    infrequent_access_ttl: Option<i32>,

    /// Number of shards
    #[serde(rename = "shardCount")]
    shard_count: i32,

    /// Whether WebTracking is enabled
    #[serde(rename = "enable_tracking")]
    enable_tracking: bool,

    /// Whether automatic shard splitting is enabled
    #[serde(rename = "autoSplit")]
    auto_split: bool,

    /// Maximum number of shards when auto-splitting
    #[serde(rename = "maxSplitShard", skip_serializing_if = "Option::is_none")]
    max_split_shard: Option<i32>,

    /// Creation time (Unix timestamp)
    #[serde(rename = "createTime")]
    create_time: i64,

    /// Last modification time (Unix timestamp)
    #[serde(rename = "lastModifyTime")]
    last_modify_time: i64,

    /// Whether metadata is appended
    #[serde(rename = "appendMeta")]
    append_meta: bool,

    /// Telemetry data type
    #[serde(rename = "telemetryType")]
    telemetry_type: String,

    /// Logstore mode (standard or query)
    mode: String,

    /// Encryption configuration
    #[serde(rename = "encrypt_conf", skip_serializing_if = "Option::is_none")]
    encrypt_conf: Option<EncryptConf>,

    /// IngestProcessor ID
    #[serde(rename = "processorId", skip_serializing_if = "Option::is_none")]
    processor_id: Option<String>,
}

impl FromHttpResponse for GetLogstoreResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        parse_json_response(body.as_ref(), http_headers)
    }
}
