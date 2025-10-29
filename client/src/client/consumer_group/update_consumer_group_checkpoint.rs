use super::*;
use crate::RequestErrorKind;
use serde::{Deserialize, Serialize};

impl crate::client::Client {
    /// Update consumer group checkpoint.
    ///
    /// This method updates the consumption checkpoint for a specific shard in a consumer group.
    /// Checkpoints track the consumption progress and are used to resume consumption from
    /// the correct position after a restart.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore containing the consumer group
    /// * `consumer_group` - The name of the consumer group
    /// * `request` - The checkpoint update request
    ///
    /// # Examples
    ///
    /// Updating a checkpoint:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    ///
    /// let resp = client
    ///     .update_consumer_group_checkpoint("my-project", "my-logstore", "my-consumer-group")
    ///     .shard_id(0)
    ///     .consumer_id("my-consumer-id")
    ///     .checkpoint("cursor-value-here")
    ///     .force_success(true)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_consumer_group_checkpoint(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        consumer_group: impl AsRef<str>,
    ) -> UpdateCheckpointRequestBuilder {
        UpdateCheckpointRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!(
                "/logstores/{}/consumergroups/{}",
                logstore.as_ref(),
                consumer_group.as_ref()
            ),
            handle: self.handle.clone(),
            consumer_id: None,
            shard_id: None,
            checkpoint: None,
            force_success: None,
        }
    }
}

pub struct UpdateCheckpointRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    consumer_id: Option<String>,
    shard_id: Option<i32>,
    checkpoint: Option<String>,
    force_success: Option<bool>,
}

impl UpdateCheckpointRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    pub fn shard_id(mut self, shard_id: i32) -> Self {
        self.shard_id = Some(shard_id);
        self
    }

    pub fn checkpoint(mut self, checkpoint: impl AsRef<str>) -> Self {
        self.checkpoint = Some(checkpoint.as_ref().to_string());
        self
    }

    pub fn force_success(mut self, force_success: bool) -> Self {
        self.force_success = Some(force_success);
        self
    }

    pub fn consumer_id(mut self, consumer_id: impl AsRef<str>) -> Self {
        self.consumer_id = Some(consumer_id.as_ref().to_string());
        self
    }

    fn build(self) -> BuildResult<UpdateCheckpointRequest> {
        check_required!(
            ("shard_id", self.shard_id),
            ("checkpoint", self.checkpoint),
            ("consumer_id", self.consumer_id)
        );

        Ok((
            self.handle,
            UpdateCheckpointRequest {
                project: self.project,
                path: self.path,
                shard_id: self.shard_id.unwrap(),
                checkpoint: self.checkpoint.unwrap(),
                force_success: self.force_success.unwrap_or(false),
                consumer_id: self.consumer_id.unwrap(),
            },
        ))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct UpdateCheckpointRequest {
    #[serde(skip_serializing)]
    project: String,
    #[serde(skip_serializing)]
    path: String,

    #[serde(skip_serializing)]
    force_success: bool,

    #[serde(skip_serializing)]
    consumer_id: String,

    #[serde(rename = "shard")]
    shard_id: i32,
    checkpoint: String,
}

impl Request for UpdateCheckpointRequest {
    type ResponseBody = ();
    const HTTP_METHOD: http::Method = http::Method::POST;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_JSON);

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn query_params(&self) -> Option<Vec<(String, String)>> {
        Some(vec![
            ("type".to_string(), "checkpoint".to_string()),
            ("consumer".to_string(), self.consumer_id.to_string()),
            ("forceSuccess".to_string(), self.force_success.to_string()),
        ])
    }

    fn body(&self) -> crate::Result<Option<bytes::Bytes>, RequestError> {
        let json = serde_json::to_string(&self).map_err(RequestErrorKind::JsonEncode)?;
        Ok(Some(bytes::Bytes::from(json)))
    }
}
