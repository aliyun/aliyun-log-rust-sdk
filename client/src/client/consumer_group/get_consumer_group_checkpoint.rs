use super::*;
use crate::ResponseResult;
use getset::Getters;
use serde::{Deserialize, Serialize};

impl crate::client::Client {
    /// Get consumer group checkpoints.
    ///
    /// This method retrieves consumption checkpoints for a consumer group.
    /// Checkpoints track the consumption progress for each shard and are used to
    /// resume consumption from the correct position after a restart.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore containing the consumer group
    /// * `consumer_group` - The name of the consumer group
    ///
    /// # Examples
    ///
    /// Get all checkpoints:
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// let resp = client.get_consumer_group_checkpoint("my-project", "my-logstore", "my-consumer-group")
    ///     .send()
    ///     .await?;
    ///
    /// for cp in resp.get_body().checkpoints() {
    ///     println!("Shard {}: {}", cp.shard_id(), cp.checkpoint());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Get checkpoint for a specific shard:
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// let resp = client.get_consumer_group_checkpoint("my-project", "my-logstore", "my-consumer-group")
    ///     .shard_id(0)  // Get checkpoint for shard 0 only
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_consumer_group_checkpoint(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        consumer_group: impl AsRef<str>,
    ) -> GetConsumerGroupCheckpointRequestBuilder {
        GetConsumerGroupCheckpointRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!(
                "/logstores/{}/consumergroups/{}",
                logstore.as_ref(),
                consumer_group.as_ref()
            ),
            handle: self.handle.clone(),
            shard_id: None,
        }
    }
}

pub struct GetConsumerGroupCheckpointRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    shard_id: Option<i32>,
}

impl GetConsumerGroupCheckpointRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<GetConsumerGroupCheckpointResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Set the shard ID to get the checkpoint for (optional).
    ///
    /// * If specified: Only the checkpoint for this shard will be returned
    /// * If not specified: Checkpoints for all shards will be returned
    ///
    /// # Arguments
    ///
    /// * `shard_id` - The ID of the shard to query
    pub fn shard_id(mut self, shard_id: i32) -> Self {
        self.shard_id = Some(shard_id);
        self
    }

    fn build(self) -> BuildResult<GetConsumerGroupCheckpointRequest> {
        Ok((
            self.handle,
            GetConsumerGroupCheckpointRequest {
                project: self.project,
                path: self.path,
                shard_id: self.shard_id,
            },
        ))
    }
}

struct GetConsumerGroupCheckpointRequest {
    project: String,
    path: String,
    shard_id: Option<i32>,
}

impl Request for GetConsumerGroupCheckpointRequest {
    type ResponseBody = GetConsumerGroupCheckpointResponse;
    const HTTP_METHOD: http::Method = http::Method::GET;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn query_params(&self) -> Option<Vec<(String, String)>> {
        if let Some(shard_id) = self.shard_id {
            Some(vec![("shard".to_string(), shard_id.to_string())])
        } else {
            None
        }
    }
}

#[derive(Debug, Default, Getters)]
pub struct GetConsumerGroupCheckpointResponse {
    #[getset(get = "pub")]
    checkpoints: Vec<ConsumerGroupCheckpoint>,
}

impl FromHttpResponse for GetConsumerGroupCheckpointResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        let checkpoints: Vec<ConsumerGroupCheckpoint> =
            parse_json_response(body.as_ref(), http_headers)?;
        Ok(GetConsumerGroupCheckpointResponse { checkpoints })
    }
}

/// Consumer group checkpoint information
#[derive(Debug, Clone, Getters, Serialize, Deserialize)]
#[getset(get = "pub")]
pub struct ConsumerGroupCheckpoint {
    /// The shard ID
    #[serde(rename = "shard")]
    shard_id: i32,
    /// The checkpoint value (cursor)
    checkpoint: String,
    /// The timestamp when this checkpoint was last updated
    #[serde(rename = "updateTime")]
    update_time: i64,
    /// The consumer that owns this checkpoint
    consumer: String,
}
