use super::*;
use crate::ResponseResult;
use getset::Getters;
use serde::{Deserialize, Serialize};

impl crate::client::Client {
    /// List all consumer groups of a logstore.
    ///
    /// This method retrieves information about all consumer groups in the specified logstore,
    /// including their timeout and ordering settings.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore to list consumer groups for
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// let resp = client.list_consumer_groups("my-project", "my-logstore")
    ///     .send()
    ///     .await?;
    ///
    /// for cg in resp.get_body().consumer_groups() {
    ///     println!("Group: {}, Timeout: {}s, Ordered: {}",
    ///         cg.consumer_group_name(),
    ///         cg.timeout(),
    ///         cg.order()
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_consumer_groups(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
    ) -> ListConsumerGroupsRequestBuilder {
        ListConsumerGroupsRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/consumergroups", logstore.as_ref()),
            handle: self.handle.clone(),
        }
    }
}

pub struct ListConsumerGroupsRequestBuilder {
    handle: HandleRef,
    project: String,
    path: String,
}

impl ListConsumerGroupsRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<ListConsumerGroupsResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<ListConsumerGroupsRequest> {
        Ok((
            self.handle,
            ListConsumerGroupsRequest {
                project: self.project,
                path: self.path,
            },
        ))
    }
}

struct ListConsumerGroupsRequest {
    project: String,
    path: String,
}

impl Request for ListConsumerGroupsRequest {
    type ResponseBody = ListConsumerGroupsResponse;
    const HTTP_METHOD: http::Method = http::Method::GET;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Default, Getters)]
pub struct ListConsumerGroupsResponse {
    #[getset(get = "pub")]
    consumer_groups: Vec<ConsumerGroup>,
}

impl FromHttpResponse for ListConsumerGroupsResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        let consumer_groups: Vec<ConsumerGroup> = parse_json_response(body.as_ref(), http_headers)?;
        Ok(ListConsumerGroupsResponse { consumer_groups })
    }
}

/// Consumer group information
#[derive(Debug, Clone, Getters, Serialize, Deserialize)]
#[getset(get = "pub")]
pub struct ConsumerGroup {
    /// Name of the consumer group
    #[serde(rename = "name")]
    consumer_group_name: String,
    /// Timeout in seconds for consumer heartbeats
    timeout: i32,
    /// Whether to consume in order (true) or allow parallel consumption (false)
    order: bool,
}
