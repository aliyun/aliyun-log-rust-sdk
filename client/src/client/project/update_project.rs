use super::*;
use crate::RequestErrorKind;
use serde::Serialize;

impl crate::client::Client {
    /// Update an existing project configuration.
    ///
    /// This method allows you to update the configuration of an existing project,
    /// such as description and recycle bin settings.
    ///
    /// # Arguments
    ///
    /// * `project_name` - The name of the project to update
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// client.update_project("test-project")
    ///     .description("updated description")
    ///     .recycle_bin_enabled(false)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_project(&self, project_name: impl AsRef<str>) -> UpdateProjectRequestBuilder {
        UpdateProjectRequestBuilder {
            project_name: project_name.as_ref().to_string(),
            handle: self.handle.clone(),
            description: None,
            recycle_bin_enabled: None,
        }
    }
}

pub struct UpdateProjectRequestBuilder {
    project_name: String,
    handle: HandleRef,
    description: Option<String>,
    recycle_bin_enabled: Option<bool>,
}

impl UpdateProjectRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Set the project description (optional).
    ///
    /// # Arguments
    ///
    /// * `description` - Updated description of the project
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set whether to enable recycle bin (optional).
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether to enable recycle bin
    pub fn recycle_bin_enabled(mut self, enabled: bool) -> Self {
        self.recycle_bin_enabled = Some(enabled);
        self
    }

    fn build(self) -> BuildResult<UpdateProjectRequest> {
        Ok((
            self.handle,
            UpdateProjectRequest {
                project_name: self.project_name,
                description: self.description,
                recycle_bin_enabled: self.recycle_bin_enabled,
            },
        ))
    }
}

#[derive(Serialize)]
struct UpdateProjectRequest {
    #[serde(skip_serializing)]
    project_name: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(rename = "recycleBinEnabled", skip_serializing_if = "Option::is_none")]
    recycle_bin_enabled: Option<bool>,
}

impl Request for UpdateProjectRequest {
    const HTTP_METHOD: http::Method = http::Method::PUT;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_JSON);
    type ResponseBody = ();

    fn project(&self) -> Option<&str> {
        Some(&self.project_name)
    }

    fn path(&self) -> &str {
        "/"
    }

    fn body(&self) -> crate::Result<Option<bytes::Bytes>, RequestError> {
        let json = serde_json::to_string(&self).map_err(RequestErrorKind::JsonEncode)?;
        Ok(Some(bytes::Bytes::from(json)))
    }
}

