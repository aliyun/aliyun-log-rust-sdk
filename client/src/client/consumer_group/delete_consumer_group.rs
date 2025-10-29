use super::*;

impl crate::client::Client {
    /// Delete a consumer group.
    ///
    /// This method deletes an existing consumer group from the logstore.
    /// All associated checkpoints will also be removed.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore containing the consumer group
    /// * `consumer_group` - The name of the consumer group to delete
    ///
    /// # Examples
    ///
    /// Deleting a consumer group:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// let resp = client
    ///     .delete_consumer_group("my-project", "my-logstore", "my-consumer-group")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_consumer_group(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        consumer_group: impl AsRef<str>,
    ) -> DeleteConsumerGroupRequestBuilder {
        DeleteConsumerGroupRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!(
                "/logstores/{}/consumergroups/{}",
                logstore.as_ref(),
                consumer_group.as_ref()
            ),
            handle: self.handle.clone(),
        }
    }
}

pub struct DeleteConsumerGroupRequestBuilder {
    handle: HandleRef,
    project: String,
    path: String,
}

impl DeleteConsumerGroupRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<DeleteConsumerGroupRequest> {
        Ok((
            self.handle,
            DeleteConsumerGroupRequest {
                project: self.project,
                path: self.path,
            },
        ))
    }
}
struct DeleteConsumerGroupRequest {
    project: String,
    path: String,
}

impl Request for DeleteConsumerGroupRequest {
    type ResponseBody = ();
    const HTTP_METHOD: http::Method = http::Method::DELETE;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }
}
