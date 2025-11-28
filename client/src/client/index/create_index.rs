use super::*;
use crate::RequestErrorKind;

impl crate::client::Client {
    /// Create a new index for a logstore.
    ///
    /// Index configuration defines how logs are indexed for querying and analysis.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project
    /// * `logstore` - The name of the logstore
    /// * `index` - Index configuration
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// use aliyun_log_rust_sdk::Index;
    ///
    /// let index = Index::new();
    /// client.create_index("my-project", "my-logstore", index)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_index(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        index: Index,
    ) -> CreateIndexRequestBuilder {
        CreateIndexRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/index", logstore.as_ref()),
            handle: self.handle.clone(),
            index,
        }
    }
}

pub struct CreateIndexRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    index: Index,
}

impl CreateIndexRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<CreateIndexRequest> {
        Ok((
            self.handle,
            CreateIndexRequest {
                project: self.project,
                path: self.path,
                index: self.index,
            },
        ))
    }
}

#[derive(Serialize)]
struct CreateIndexRequest {
    #[serde(skip_serializing)]
    project: String,

    #[serde(skip_serializing)]
    path: String,

    #[serde(flatten)]
    index: Index,
}

impl Request for CreateIndexRequest {
    const HTTP_METHOD: http::Method = http::Method::POST;
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
