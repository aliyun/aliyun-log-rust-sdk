use super::*;
use getset::Getters;
use serde::Deserialize;

impl crate::client::Client {
    /// Get project details.
    ///
    /// This method retrieves detailed information about a project, including its configuration,
    /// creation time, and resource settings.
    ///
    /// # Arguments
    ///
    /// * `project_name` - The name of the project to get
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// let response = client.get_project("test-project")
    ///     .send()
    ///     .await?;
    /// println!("Project description: {}", response.body().description());
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_project(&self, project_name: impl AsRef<str>) -> GetProjectRequestBuilder {
        GetProjectRequestBuilder {
            project_name: project_name.as_ref().to_string(),
            handle: self.handle.clone(),
        }
    }
}

pub struct GetProjectRequestBuilder {
    handle: HandleRef,
    project_name: String,
}

impl GetProjectRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<GetProjectResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<GetProjectRequest> {
        Ok((
            self.handle,
            GetProjectRequest {
                project_name: self.project_name,
            },
        ))
    }
}

struct GetProjectRequest {
    project_name: String,
}

impl Request for GetProjectRequest {
    type ResponseBody = GetProjectResponse;
    const HTTP_METHOD: http::Method = http::Method::GET;

    fn project(&self) -> Option<&str> {
        Some(&self.project_name)
    }

    fn path(&self) -> &str {
        "/"
    }
}

/// Project information
#[derive(Debug, Getters, Deserialize)]
#[getset(get = "pub")]
pub struct GetProjectResponse {
    /// Project name
    #[serde(rename = "projectName")]
    project_name: String,

    /// Project status
    status: String,

    /// Project owner ID
    owner: String,

    /// Project description
    description: String,

    /// Creation time
    #[serde(rename = "createTime")]
    create_time: String,

    /// Last modification time
    #[serde(rename = "lastModifyTime")]
    last_modify_time: String,

    /// Region where the project is located
    #[serde(skip_serializing_if = "Option::is_none")]
    region: Option<String>,

    /// Specific location of the project
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<String>,

    /// Resource group ID
    #[serde(rename = "resourceGroupId", skip_serializing_if = "Option::is_none")]
    resource_group_id: Option<String>,

    /// Data redundancy type (LRS or ZRS)
    #[serde(rename = "dataRedundancyType")]
    data_redundancy_type: String,

    /// Transfer acceleration status
    #[serde(
        rename = "transferAcceleration",
        skip_serializing_if = "Option::is_none"
    )]
    transfer_acceleration: Option<String>,

    /// Whether recycle bin is enabled
    #[serde(rename = "recycleBinEnabled", skip_serializing_if = "Option::is_none")]
    recycle_bin_enabled: Option<bool>,

    /// Whether deletion protection is enabled
    #[serde(rename = "deletionProtection", skip_serializing_if = "Option::is_none")]
    deletion_protection: Option<bool>,
}

impl FromHttpResponse for GetProjectResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        parse_json_response(body.as_ref(), http_headers)
    }
}
