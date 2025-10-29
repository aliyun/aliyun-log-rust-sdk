use super::*;
use crate::RequestErrorKind;
use getset::Getters;
use serde::Deserialize;

impl crate::client::Client {
    /// Send a heartbeat for a consumer group.
    ///
    /// Consumer processes should send heartbeats regularly to indicate they are still active.
    /// If a consumer fails to send heartbeats within the timeout period, its shards may be
    /// reassigned to other consumers.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore containing the consumer group
    /// * `consumer_group` - The name of the consumer group
    ///
    /// # Examples
    ///
    /// Sending a heartbeat:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// let resp = client
    ///     .consumer_group_heartbeat("my-project", "my-logstore", "my-consumer-group")
    ///     .consumer("consumer-1") // required, the consumerId in the consumer group
    ///     .shards(vec![1, 2, 3]) // optional, the current held shards of the consumer
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn consumer_group_heartbeat(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        consumer_group: impl AsRef<str>,
    ) -> ConsumerGroupHeartbeatRequestBuilder {
        ConsumerGroupHeartbeatRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!(
                "/logstores/{}/consumergroups/{}",
                logstore.as_ref(),
                consumer_group.as_ref()
            ),
            handle: self.handle.clone(),
            consumer: None,
            shards: vec![],
        }
    }
}

pub struct ConsumerGroupHeartbeatRequestBuilder {
    handle: HandleRef,
    project: String,
    path: String,
    consumer: Option<String>,
    shards: Vec<i32>,
}

impl ConsumerGroupHeartbeatRequestBuilder {
    pub fn send(self) -> ResponseResultBoxFuture<ConsumerGroupHeartbeatResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Set the consumer identifier (required).
    ///
    /// This is the unique identifier of the consumer within the consumer group.
    pub fn consumer(mut self, consumer: impl AsRef<str>) -> Self {
        self.consumer = Some(consumer.as_ref().to_string());
        self
    }

    /// Set the list of shards currently held by this consumer (optional).
    ///
    /// This allows the server to track which shards are assigned to this consumer.
    /// If not specified, an empty list will be sent.
    pub fn shards(mut self, shards: Vec<i32>) -> Self {
        self.shards = shards;
        self
    }

    fn build(self) -> BuildResult<ConsumerGroupHeartbeatRequest> {
        check_required!(("consumer", self.consumer));
        Ok((
            self.handle,
            ConsumerGroupHeartbeatRequest {
                project: self.project,
                path: self.path,
                consumer: self.consumer.unwrap(),
                shards: self.shards,
            },
        ))
    }
}

struct ConsumerGroupHeartbeatRequest {
    project: String,
    path: String,
    consumer: String,
    shards: Vec<i32>,
}

impl Request for ConsumerGroupHeartbeatRequest {
    const HTTP_METHOD: http::Method = http::Method::POST;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_JSON);
    type ResponseBody = ConsumerGroupHeartbeatResponse;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn query_params(&self) -> Option<Vec<(String, String)>> {
        Some(vec![
            ("consumer".to_string(), self.consumer.clone()),
            ("type".to_string(), "heartbeat".to_string()),
        ])
    }

    fn body(&self) -> crate::Result<Option<bytes::Bytes>, RequestError> {
        let json = serde_json::to_string(&self.shards).map_err(RequestErrorKind::JsonEncode)?;
        Ok(Some(bytes::Bytes::from(json)))
    }
}

#[derive(Debug, Deserialize, Getters)]
pub struct ConsumerGroupHeartbeatResponse {
    #[getset(get = "pub")]
    /// The assigned shards of the consumer
    shards: Vec<i32>,
}

impl FromHttpResponse for ConsumerGroupHeartbeatResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        let shards = parse_json_response(body.as_ref(), http_headers)?;
        Ok(ConsumerGroupHeartbeatResponse { shards })
    }
}
