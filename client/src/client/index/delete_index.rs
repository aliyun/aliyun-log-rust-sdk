use super::*;

impl crate::client::Client {
    /// Delete index configuration for a logstore.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project
    /// * `logstore` - The name of the logstore
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// client.delete_index("my-project", "my-logstore")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_index(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
    ) -> DeleteIndexRequestBuilder {
        DeleteIndexRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/index", logstore.as_ref()),
            handle: self.handle.clone(),
        }
    }
}

pub struct DeleteIndexRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
}

impl DeleteIndexRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<DeleteIndexRequest> {
        Ok((
            self.handle,
            DeleteIndexRequest {
                project: self.project,
                path: self.path,
            },
        ))
    }
}

struct DeleteIndexRequest {
    project: String,
    path: String,
}

impl Request for DeleteIndexRequest {
    type ResponseBody = ();
    const HTTP_METHOD: http::Method = http::Method::DELETE;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }
}
