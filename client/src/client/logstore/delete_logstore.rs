use super::*;

impl crate::client::Client {
    /// Delete a logstore.
    ///
    /// This method deletes an existing logstore and all its associated data.
    /// This operation is irreversible, so use it with caution.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore_name` - The name of the logstore to delete
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// client.delete_logstore("my-project", "my-logstore")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_logstore(
        &self,
        project: impl AsRef<str>,
        logstore_name: impl AsRef<str>,
    ) -> DeleteLogstoreRequestBuilder {
        DeleteLogstoreRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}", logstore_name.as_ref()),
            handle: self.handle.clone(),
        }
    }
}

pub struct DeleteLogstoreRequestBuilder {
    handle: HandleRef,
    project: String,
    path: String,
}

impl DeleteLogstoreRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<DeleteLogstoreRequest> {
        Ok((
            self.handle,
            DeleteLogstoreRequest {
                project: self.project,
                path: self.path,
            },
        ))
    }
}

struct DeleteLogstoreRequest {
    project: String,
    path: String,
}

impl Request for DeleteLogstoreRequest {
    type ResponseBody = ();
    const HTTP_METHOD: http::Method = http::Method::DELETE;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }
}
