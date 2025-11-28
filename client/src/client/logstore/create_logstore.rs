use super::*;
use crate::RequestErrorKind;
use serde::{Deserialize, Serialize};

impl crate::client::Client {
    /// Create a new logstore in a project.
    ///
    /// A logstore is a unit for log storage, query, and analysis in Log Service.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project
    /// * `logstore_name` - The name of the logstore to create. Naming rules:
    ///   - Unique within the same project
    ///   - Only lowercase letters, numbers, hyphens (-), and underscores (_)
    ///   - Must start and end with lowercase letter or number
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// client.create_logstore("my-project", "my-logstore")
    ///     .shard_count(2)
    ///     .ttl(30)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_logstore(
        &self,
        project: impl AsRef<str>,
        logstore_name: impl AsRef<str>,
    ) -> CreateLogstoreRequestBuilder {
        CreateLogstoreRequestBuilder {
            project: project.as_ref().to_string(),
            handle: self.handle.clone(),
            logstore_name: logstore_name.as_ref().to_string(),
            shard_count: None,
            ttl: None,
            encrypt_conf: None,
            auto_split: None,
            enable_tracking: None,
            max_split_shard: None,
            append_meta: None,
            telemetry_type: None,
            hot_ttl: None,
            mode: None,
            infrequent_access_ttl: None,
            processor_id: None,
        }
    }
}

pub struct CreateLogstoreRequestBuilder {
    project: String,
    handle: HandleRef,
    logstore_name: String,
    shard_count: Option<i32>,
    ttl: Option<i32>,
    encrypt_conf: Option<EncryptConf>,
    auto_split: Option<bool>,
    enable_tracking: Option<bool>,
    max_split_shard: Option<i32>,
    append_meta: Option<bool>,
    telemetry_type: Option<String>,
    hot_ttl: Option<i32>,
    mode: Option<String>,
    infrequent_access_ttl: Option<i32>,
    processor_id: Option<String>,
}

impl CreateLogstoreRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Set the number of shards (required).
    ///
    /// # Arguments
    ///
    /// * `count` - Number of shards, minimum 1, maximum 256
    pub fn shard_count(mut self, count: i32) -> Self {
        self.shard_count = Some(count);
        self
    }

    /// Set the data retention time in days (required).
    ///
    /// # Arguments
    ///
    /// * `days` - Retention time in days, range: 1-3650. Use 3650 for permanent storage
    pub fn ttl(mut self, days: i32) -> Self {
        self.ttl = Some(days);
        self
    }

    /// Set encryption configuration (optional).
    ///
    /// # Arguments
    ///
    /// * `encrypt_conf` - Encryption configuration
    pub fn encrypt_conf(mut self, encrypt_conf: EncryptConf) -> Self {
        self.encrypt_conf = Some(encrypt_conf);
        self
    }

    /// Set whether to automatically split shards (optional).
    ///
    /// # Arguments
    ///
    /// * `enabled` - Enable automatic shard splitting
    pub fn auto_split(mut self, enabled: bool) -> Self {
        self.auto_split = Some(enabled);
        self
    }

    /// Set whether to enable WebTracking (optional).
    ///
    /// # Arguments
    ///
    /// * `enabled` - Enable WebTracking
    pub fn enable_tracking(mut self, enabled: bool) -> Self {
        self.enable_tracking = Some(enabled);
        self
    }

    /// Set the maximum number of shards when auto-splitting (optional).
    ///
    /// Required when auto_split is true. Range: 1-256.
    ///
    /// # Arguments
    ///
    /// * `max` - Maximum number of shards
    pub fn max_split_shard(mut self, max: i32) -> Self {
        self.max_split_shard = Some(max);
        self
    }

    /// Set whether to append metadata (optional).
    ///
    /// When enabled, automatically adds public IP and log arrival time to Tag field.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Enable metadata appending
    pub fn append_meta(mut self, enabled: bool) -> Self {
        self.append_meta = Some(enabled);
        self
    }

    /// Set the telemetry data type (optional).
    ///
    /// # Arguments
    ///
    /// * `telemetry_type` - Type of telemetry data. Valid values:
    ///   - `None`: Log data (default)
    ///   - `Metrics`: Time series data
    pub fn telemetry_type(mut self, telemetry_type: impl Into<String>) -> Self {
        self.telemetry_type = Some(telemetry_type.into());
        self
    }

    /// Set hot storage TTL in days (optional).
    ///
    /// Data older than this will be moved to infrequent access storage.
    /// Range: minimum 7, maximum ttl value. Use -1 to keep all data in hot storage.
    ///
    /// # Arguments
    ///
    /// * `days` - Hot storage TTL in days
    pub fn hot_ttl(mut self, days: i32) -> Self {
        self.hot_ttl = Some(days);
        self
    }

    /// Set the logstore mode (optional).
    ///
    /// # Arguments
    ///
    /// * `mode` - Logstore mode. Valid values:
    ///   - `standard`: Standard mode with full query and analysis features
    ///   - `query`: Query mode with high-performance queries but no SQL analysis
    pub fn mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = Some(mode.into());
        self
    }

    /// Set infrequent access TTL in days (optional).
    ///
    /// Minimum 30 days before transitioning to archive storage.
    ///
    /// # Arguments
    ///
    /// * `days` - Infrequent access TTL in days
    pub fn infrequent_access_ttl(mut self, days: i32) -> Self {
        self.infrequent_access_ttl = Some(days);
        self
    }

    /// Set the IngestProcessor ID (optional).
    ///
    /// # Arguments
    ///
    /// * `processor_id` - IngestProcessor ID
    pub fn processor_id(mut self, processor_id: impl Into<String>) -> Self {
        self.processor_id = Some(processor_id.into());
        self
    }

    fn build(self) -> BuildResult<CreateLogstoreRequest> {
        check_required!(("shard_count", self.shard_count), ("ttl", self.ttl));

        Ok((
            self.handle,
            CreateLogstoreRequest {
                project: self.project,
                logstore_name: self.logstore_name,
                shard_count: self.shard_count.unwrap(),
                ttl: self.ttl.unwrap(),
                encrypt_conf: self.encrypt_conf,
                auto_split: self.auto_split,
                enable_tracking: self.enable_tracking,
                max_split_shard: self.max_split_shard,
                append_meta: self.append_meta,
                telemetry_type: self.telemetry_type,
                hot_ttl: self.hot_ttl,
                mode: self.mode,
                infrequent_access_ttl: self.infrequent_access_ttl,
                processor_id: self.processor_id,
            },
        ))
    }
}

/// Encryption configuration for logstore
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptConf {
    /// Whether to enable encryption
    pub enable: bool,

    /// Encryption algorithm type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encrypt_type: Option<String>,

    /// User-provided CMK configuration for BYOK
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_cmk_info: Option<EncryptUserCmkConf>,
}

impl EncryptConf {
    /// Create a new encryption configuration
    ///
    /// # Arguments
    ///
    /// * `enable` - Whether to enable encryption
    /// * `encrypt_type` - Encryption algorithm type. Valid values:
    ///   default, m4, sm4_ecb, sm4_cbc, sm4_gcm, aes_ecb, aes_cbc, aes_cfb, aes_ofb, aes_gcm
    pub fn new(enable: bool, encrypt_type: impl Into<String>) -> Self {
        Self {
            enable,
            encrypt_type: Some(encrypt_type.into()),
            user_cmk_info: None,
        }
    }

    /// Set user-provided CMK configuration for BYOK (Bring Your Own Key)
    pub fn with_user_cmk(mut self, user_cmk_info: EncryptUserCmkConf) -> Self {
        self.user_cmk_info = Some(user_cmk_info);
        self
    }
}

/// User-provided CMK configuration for BYOK
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptUserCmkConf {
    /// CMK (Customer Master Key) ID
    pub cmk_key_id: String,

    /// RAM role ARN
    pub arn: String,

    /// Region ID where the CMK is located
    pub region_id: String,
}

impl EncryptUserCmkConf {
    /// Create a new user CMK configuration
    ///
    /// # Arguments
    ///
    /// * `cmk_key_id` - CMK (Customer Master Key) ID
    /// * `arn` - RAM role ARN
    /// * `region_id` - Region ID where the CMK is located
    pub fn new(
        cmk_key_id: impl Into<String>,
        arn: impl Into<String>,
        region_id: impl Into<String>,
    ) -> Self {
        Self {
            cmk_key_id: cmk_key_id.into(),
            arn: arn.into(),
            region_id: region_id.into(),
        }
    }
}

#[derive(Serialize)]
struct CreateLogstoreRequest {
    #[serde(skip_serializing)]
    project: String,

    #[serde(rename = "logstoreName")]
    logstore_name: String,

    #[serde(rename = "shardCount")]
    shard_count: i32,

    ttl: i32,

    #[serde(rename = "encrypt_conf", skip_serializing_if = "Option::is_none")]
    encrypt_conf: Option<EncryptConf>,

    #[serde(rename = "autoSplit", skip_serializing_if = "Option::is_none")]
    auto_split: Option<bool>,

    #[serde(rename = "enable_tracking", skip_serializing_if = "Option::is_none")]
    enable_tracking: Option<bool>,

    #[serde(rename = "maxSplitShard", skip_serializing_if = "Option::is_none")]
    max_split_shard: Option<i32>,

    #[serde(rename = "appendMeta", skip_serializing_if = "Option::is_none")]
    append_meta: Option<bool>,

    #[serde(rename = "telemetryType", skip_serializing_if = "Option::is_none")]
    telemetry_type: Option<String>,

    #[serde(rename = "hot_ttl", skip_serializing_if = "Option::is_none")]
    hot_ttl: Option<i32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    mode: Option<String>,

    #[serde(
        rename = "infrequentAccessTTL",
        skip_serializing_if = "Option::is_none"
    )]
    infrequent_access_ttl: Option<i32>,

    #[serde(rename = "processorId", skip_serializing_if = "Option::is_none")]
    processor_id: Option<String>,
}

impl Request for CreateLogstoreRequest {
    const HTTP_METHOD: http::Method = http::Method::POST;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_JSON);
    type ResponseBody = ();

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        "/logstores"
    }

    fn body(&self) -> crate::Result<Option<bytes::Bytes>, RequestError> {
        let json = serde_json::to_string(&self).map_err(RequestErrorKind::JsonEncode)?;
        Ok(Some(bytes::Bytes::from(json)))
    }
}
