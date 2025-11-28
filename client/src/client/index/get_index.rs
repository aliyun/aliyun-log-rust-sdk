use super::*;
use crate::ResponseResult;

impl crate::client::Client {
    /// Get index configuration for a logstore.
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
    /// let response = client.get_index("my-project", "my-logstore")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_index(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
    ) -> GetIndexRequestBuilder {
        GetIndexRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/index", logstore.as_ref()),
            handle: self.handle.clone(),
        }
    }
}

pub struct GetIndexRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
}

impl GetIndexRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<Index> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<GetIndexRequest> {
        Ok((
            self.handle,
            GetIndexRequest {
                project: self.project,
                path: self.path,
            },
        ))
    }
}

struct GetIndexRequest {
    project: String,
    path: String,
}

impl Request for GetIndexRequest {
    type ResponseBody = Index;
    const HTTP_METHOD: http::Method = http::Method::GET;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }
}

impl FromHttpResponse for Index {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        parse_json_response(body.as_ref(), http_headers)
    }
}
