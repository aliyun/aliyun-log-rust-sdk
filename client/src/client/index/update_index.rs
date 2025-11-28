use super::*;
use crate::RequestErrorKind;

impl crate::client::Client {
    /// Update an existing index configuration.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project
    /// * `logstore` - The name of the logstore
    /// * `index` - Updated index configuration
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// use aliyun_log_rust_sdk::Index;
    ///
    /// let index = Index::new();
    /// client.update_index("my-project", "my-logstore", index)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_index(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        index: Index,
    ) -> UpdateIndexRequestBuilder {
        UpdateIndexRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/index", logstore.as_ref()),
            handle: self.handle.clone(),
            index,
        }
    }
}

pub struct UpdateIndexRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    index: Index,
}

impl UpdateIndexRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<UpdateIndexRequest> {
        Ok((
            self.handle,
            UpdateIndexRequest {
                project: self.project,
                path: self.path,
                index: self.index,
            },
        ))
    }
}

#[derive(Serialize)]
struct UpdateIndexRequest {
    #[serde(skip_serializing)]
    project: String,

    #[serde(skip_serializing)]
    path: String,

    #[serde(flatten)]
    index: Index,
}

impl Request for UpdateIndexRequest {
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
