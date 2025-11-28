use super::*;

impl crate::client::Client {
    /// Delete a project.
    ///
    /// This method deletes an existing project and all its associated resources.
    /// This operation is irreversible, so use it with caution.
    ///
    /// # Arguments
    ///
    /// * `project_name` - The name of the project to delete
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// client.delete_project("test-project")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete_project(&self, project_name: impl AsRef<str>) -> DeleteProjectRequestBuilder {
        DeleteProjectRequestBuilder {
            project_name: project_name.as_ref().to_string(),
            handle: self.handle.clone(),
        }
    }
}

pub struct DeleteProjectRequestBuilder {
    handle: HandleRef,
    project_name: String,
}

impl DeleteProjectRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<DeleteProjectRequest> {
        Ok((
            self.handle,
            DeleteProjectRequest {
                project_name: self.project_name,
            },
        ))
    }
}

struct DeleteProjectRequest {
    project_name: String,
}

impl Request for DeleteProjectRequest {
    type ResponseBody = ();
    const HTTP_METHOD: http::Method = http::Method::DELETE;

    fn project(&self) -> Option<&str> {
        Some(&self.project_name)
    }

    fn path(&self) -> &str {
        "/"
    }
}
