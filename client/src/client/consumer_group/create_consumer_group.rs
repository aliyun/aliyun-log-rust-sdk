use super::*;
use crate::RequestErrorKind;
use serde::Serialize;

impl crate::client::Client {
    /// Create a consumer group for a logstore.
    ///
    /// Consumer groups enable multiple consumers to read data from a logstore in a coordinated way.
    /// Each consumer group maintains checkpoints for each shard to track consumption progress.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore to create consumer group for
    /// * `consumer_group` - The name of the consumer group to create
    ///
    /// # Examples
    ///
    /// Creating a consumer group:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    ///
    ///
    /// let resp = client
    ///     .create_consumer_group("my-project", "my-logstore", "my-consumer-group")
    ///     .timeout(60) // Required, the heartbeat timeout in seconds
    ///     .order(true) // Required, whether to consume in order
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_consumer_group(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        consumer_group: impl AsRef<str>,
    ) -> CreateConsumerGroupRequestBuilder {
        CreateConsumerGroupRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/consumergroups", logstore.as_ref()),
            handle: self.handle.clone(),
            consumer_group: consumer_group.as_ref().to_string(),
            timeout: None,
            order: None,
        }
    }
}

pub struct CreateConsumerGroupRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    consumer_group: String,
    timeout: Option<i32>,
    order: Option<bool>,
}

impl CreateConsumerGroupRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Required, the heartbeat timeout in seconds
    pub fn timeout(mut self, timeout: i32) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Required, whether to consume in order
    pub fn order(mut self, order: bool) -> Self {
        self.order = Some(order);
        self
    }

    fn build(self) -> BuildResult<CreateConsumerGroupRequest> {
        check_required!(("timeout", self.timeout), ("order", self.order));
        Ok((
            self.handle,
            CreateConsumerGroupRequest {
                project: self.project,
                path: self.path,
                consumer_group: self.consumer_group,
                timeout: self.timeout.unwrap(),
                order: self.order.unwrap(),
            },
        ))
    }
}

#[derive(Serialize)]
struct CreateConsumerGroupRequest {
    #[serde(skip_serializing)]
    project: String,
    #[serde(skip_serializing)]
    path: String,

    #[serde(rename = "consumerGroup")]
    consumer_group: String,
    timeout: i32,
    order: bool,
}

impl Request for CreateConsumerGroupRequest {
    type ResponseBody = ();
    const HTTP_METHOD: http::Method = http::Method::POST;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_JSON);

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
