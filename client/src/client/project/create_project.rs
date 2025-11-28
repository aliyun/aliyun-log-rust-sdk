use super::*;
use crate::RequestErrorKind;
use serde::Serialize;

impl crate::client::Client {
    /// Create a new project.
    ///
    /// Project names must be globally unique within the Alibaba Cloud region and cannot be modified after creation.
    ///
    /// # Arguments
    ///
    /// * `project_name` - Name of the project to create. Must be globally unique and follow naming rules:
    ///   - Only lowercase letters, numbers, and hyphens (-)
    ///   - Must start and end with lowercase letter or number
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// client.create_project("test-project")
    ///     .description("this is test")
    ///     .resource_group_id("rg-aekzf******sxby")
    ///     .data_redundancy_type("LRS")
    ///     .recycle_bin_enabled(true)
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create_project(&self, project_name: impl AsRef<str>) -> CreateProjectRequestBuilder {
        CreateProjectRequestBuilder {
            handle: self.handle.clone(),
            project_name: project_name.as_ref().to_string(),
            description: None,
            resource_group_id: None,
            data_redundancy_type: None,
            recycle_bin_enabled: None,
        }
    }
}

pub struct CreateProjectRequestBuilder {
    handle: HandleRef,
    project_name: String,
    description: Option<String>,
    resource_group_id: Option<String>,
    data_redundancy_type: Option<String>,
    recycle_bin_enabled: Option<bool>,
}

impl CreateProjectRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<()> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Set the project description (required).
    ///
    /// # Arguments
    ///
    /// * `description` - Description of the project
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the resource group ID (optional).
    ///
    /// # Arguments
    ///
    /// * `resource_group_id` - Resource group ID to create project into.
    pub fn resource_group_id(mut self, resource_group_id: impl Into<String>) -> Self {
        self.resource_group_id = Some(resource_group_id.into());
        self
    }

    /// Set the data redundancy type (optional).
    ///
    /// # Arguments
    ///
    /// * `data_redundancy_type` - Data redundancy type. Valid values:
    ///   - `LRS`: Local redundant storage
    ///   - `ZRS`: Zone redundant storage
    pub fn data_redundancy_type(mut self, data_redundancy_type: impl Into<String>) -> Self {
        self.data_redundancy_type = Some(data_redundancy_type.into());
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

    fn build(self) -> BuildResult<CreateProjectRequest> {
        check_required!(("description", self.description));
        Ok((
            self.handle,
            CreateProjectRequest {
                project_name: self.project_name,
                description: self.description.unwrap(),
                resource_group_id: self.resource_group_id,
                data_redundancy_type: self.data_redundancy_type,
                recycle_bin_enabled: self.recycle_bin_enabled,
            },
        ))
    }
}

#[derive(Serialize)]
struct CreateProjectRequest {
    #[serde(rename = "projectName")]
    project_name: String,
    description: String,
    #[serde(rename = "resourceGroupId", skip_serializing_if = "Option::is_none")]
    resource_group_id: Option<String>,
    #[serde(rename = "dataRedundancyType", skip_serializing_if = "Option::is_none")]
    data_redundancy_type: Option<String>,
    #[serde(rename = "recycleBinEnabled", skip_serializing_if = "Option::is_none")]
    recycle_bin_enabled: Option<bool>,
}

impl Request for CreateProjectRequest {
    const HTTP_METHOD: http::Method = http::Method::POST;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_JSON);
    type ResponseBody = ();

    fn project(&self) -> Option<&str> {
        None
    }

    fn path(&self) -> &str {
        "/"
    }

    fn body(&self) -> crate::Result<Option<bytes::Bytes>, RequestError> {
        let json = serde_json::to_string(&self).map_err(RequestErrorKind::JsonEncode)?;
        Ok(Some(bytes::Bytes::from(json)))
    }
}

