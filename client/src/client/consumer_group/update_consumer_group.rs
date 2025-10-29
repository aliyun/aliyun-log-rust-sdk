use super::*;
use crate::RequestErrorKind;
use serde::Serialize;

impl crate::client::Client {
    /// Update a consumer group configuration.
    ///
    /// This method allows you to update the configuration of an existing consumer group,
    /// such as timeout and ordering settings.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore containing the consumer group
    /// * `consumer_group` - The name of the consumer group to update
    ///
    /// # Examples
    ///
    /// Updating a consumer group:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// let resp = client
    ///     .update_consumer_group("my-project", "my-logstore", "my-consumer-group")
    ///     .timeout(60) // Required, the heartbeat timeout in seconds
    ///     .order(false) // Required, whether to consume in order
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_consumer_group(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        consumer_group: impl AsRef<str>,
    ) -> UpdateConsumerGroupRequestBuilder {
        UpdateConsumerGroupRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!(
                "/logstores/{}/consumergroups/{}",
                logstore.as_ref(),
                consumer_group.as_ref()
            ),
            handle: self.handle.clone(),
            timeout: None,
            order: None,
        }
    }
}

pub struct UpdateConsumerGroupRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    timeout: Option<i32>,
    order: Option<bool>,
}

impl UpdateConsumerGroupRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Set the heartbeat timeout in seconds (required).
    ///
    /// Consumers must send heartbeats within this timeout period to maintain ownership of shards.
    /// If a consumer fails to send heartbeats within this period, its shards may be reassigned.
    pub fn timeout(mut self, timeout: i32) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set whether to consume logs in order (required).
    ///
    /// When set to `true`, new shards will not be assigned until the shard they are split from is consumed completely.
    /// When set to `false`, the shards will be assigned instantly after creation.
    pub fn order(mut self, order: bool) -> Self {
        self.order = Some(order);
        self
    }

    fn build(self) -> BuildResult<UpdateConsumerGroupRequest> {
        check_required!(("timeout", self.timeout), ("order", self.order));

        Ok((
            self.handle,
            UpdateConsumerGroupRequest {
                project: self.project,
                path: self.path,
                timeout: self.timeout.unwrap(),
                order: self.order.unwrap(),
            },
        ))
    }
}

#[derive(Serialize)]
struct UpdateConsumerGroupRequest {
    #[serde(skip_serializing)]
    project: String,
    #[serde(skip_serializing)]
    path: String,

    timeout: i32,
    order: bool,
}

impl Request for UpdateConsumerGroupRequest {
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
