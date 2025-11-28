use super::*;
use crate::RequestErrorKind;
use serde::Serialize;

impl crate::client::Client {
    /// Update an existing logstore configuration.
    ///
    /// This method allows you to update the configuration of an existing logstore,
    /// such as TTL, encryption, and storage settings.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore_name` - The name of the logstore to update
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// client.update_logstore("my-project", "my-logstore")
    ///     .ttl(90)
    ///     .hot_ttl(30)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_logstore(
        &self,
        project: impl AsRef<str>,
        logstore_name: impl AsRef<str>,
    ) -> UpdateLogstoreRequestBuilder {
        UpdateLogstoreRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}", logstore_name.as_ref()),
            handle: self.handle.clone(),
            ttl: None,
            encrypt_conf: None,
            auto_split: None,
            enable_tracking: None,
            max_split_shard: None,
            append_meta: None,
            hot_ttl: None,
            mode: None,
            infrequent_access_ttl: None,
            processor_id: None,
        }
    }
}

pub struct UpdateLogstoreRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    ttl: Option<i32>,
    encrypt_conf: Option<EncryptConf>,
    auto_split: Option<bool>,
    enable_tracking: Option<bool>,
    max_split_shard: Option<i32>,
    append_meta: Option<bool>,
    hot_ttl: Option<i32>,
    mode: Option<String>,
    infrequent_access_ttl: Option<i32>,
    processor_id: Option<String>,
}

impl UpdateLogstoreRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Set the data retention time in days (required).
    ///
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

    fn build(self) -> BuildResult<UpdateLogstoreRequest> {
        Ok((
            self.handle,
            UpdateLogstoreRequest {
                project: self.project,
                path: self.path,
                ttl: self.ttl,
                encrypt_conf: self.encrypt_conf,
                auto_split: self.auto_split,
                enable_tracking: self.enable_tracking,
                max_split_shard: self.max_split_shard,
                append_meta: self.append_meta,
                hot_ttl: self.hot_ttl,
                mode: self.mode,
                infrequent_access_ttl: self.infrequent_access_ttl,
                processor_id: self.processor_id,
            },
        ))
    }
}

#[derive(Serialize)]
struct UpdateLogstoreRequest {
    #[serde(skip_serializing)]
    project: String,

    #[serde(skip_serializing)]
    path: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    ttl: Option<i32>,

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

impl Request for UpdateLogstoreRequest {
    const HTTP_METHOD: http::Method = http::Method::PUT;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_JSON);
    type ResponseBody = ();

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> crate::Result<Option<bytes::Bytes>, RequestError> {
        let json = serde_json::to_string(&self).map_err(RequestErrorKind::JsonEncode)?;
        Ok(Some(bytes::Bytes::from(json)))
    }
}
