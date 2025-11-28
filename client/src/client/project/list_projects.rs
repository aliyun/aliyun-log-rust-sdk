use super::*;
use crate::ResponseResult;
use getset::Getters;
use serde::Deserialize;

impl crate::client::Client {
    /// List projects with pagination and filtering.
    ///
    /// This method retrieves a list of projects with support for pagination and filtering
    /// by project name, description, and resource group ID.
    ///
    /// # Arguments
    ///
    /// * `offset` - The offset for pagination (starting from 0)
    /// * `size` - The number of projects to return (page size)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// // List first 10 projects
    /// let resp = client.list_projects(0, 10)
    ///     .send()
    ///     .await?;
    ///
    /// println!("Total projects: {}", resp.body().total());
    /// for project in resp.body().projects() {
    ///     println!("Project: {}", project.project_name());
    /// }
    ///
    /// // Filter by project name
    /// let resp = client.list_projects(0, 10)
    ///     .project_name("test") // search by name
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_projects(&self, offset: i32, size: i32) -> ListProjectsRequestBuilder {
        ListProjectsRequestBuilder {
            handle: self.handle.clone(),
            offset,
            size,
            project_name: None,
            description: None,
            resource_group_id: None,
        }
    }
}

pub struct ListProjectsRequestBuilder {
    handle: HandleRef,
    offset: i32,
    size: i32,
    project_name: Option<String>,
    description: Option<String>,
    resource_group_id: Option<String>,
}

impl ListProjectsRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<ListProjectsResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Filter projects by name (fuzzy search).
    ///
    /// # Arguments
    ///
    /// * `project_name` - Project name to search for (supports partial matching)
    pub fn project_name(mut self, project_name: impl Into<String>) -> Self {
        self.project_name = Some(project_name.into());
        self
    }

    /// Filter projects by description (fuzzy search).
    ///
    /// # Arguments
    ///
    /// * `description` - Description to search for (supports partial matching)
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Filter projects by resource group ID.
    ///
    /// # Arguments
    ///
    /// * `resource_group_id` - Resource group ID to filter by
    pub fn resource_group_id(mut self, resource_group_id: impl Into<String>) -> Self {
        self.resource_group_id = Some(resource_group_id.into());
        self
    }

    fn build(self) -> BuildResult<ListProjectsRequest> {
        Ok((
            self.handle,
            ListProjectsRequest {
                offset: self.offset,
                size: self.size,
                project_name: self.project_name,
                description: self.description,
                resource_group_id: self.resource_group_id,
            },
        ))
    }
}

struct ListProjectsRequest {
    offset: i32,
    size: i32,
    project_name: Option<String>,
    description: Option<String>,
    resource_group_id: Option<String>,
}

impl Request for ListProjectsRequest {
    type ResponseBody = ListProjectsResponse;
    const HTTP_METHOD: http::Method = http::Method::GET;

    fn project(&self) -> Option<&str> {
        None
    }

    fn path(&self) -> &str {
        "/"
    }

    fn query_params(&self) -> Option<Vec<(String, String)>> {
        let mut params = vec![
            ("offset".to_string(), self.offset.to_string()),
            ("size".to_string(), self.size.to_string()),
        ];

        if let Some(ref project_name) = self.project_name {
            params.push(("projectName".to_string(), project_name.clone()));
        }

        if let Some(ref description) = self.description {
            params.push(("description".to_string(), description.clone()));
        }

        if let Some(ref resource_group_id) = self.resource_group_id {
            params.push(("resourceGroupId".to_string(), resource_group_id.clone()));
        }

        Some(params)
    }
}

/// Response containing a list of projects
#[derive(Debug, Getters, Deserialize)]
#[getset(get = "pub")]
pub struct ListProjectsResponse {
    /// Number of projects returned in this response
    count: i32,

    /// Total number of projects matching the filter criteria
    total: i32,

    /// List of projects
    projects: Vec<ListProjectsProject>,
}

impl FromHttpResponse for ListProjectsResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        parse_json_response(body.as_ref(), http_headers)
    }
}

/// Project information in list response
#[derive(Debug, Clone, Getters, Deserialize)]
#[getset(get = "pub")]
pub struct ListProjectsProject {
    /// Project name
    #[serde(rename = "projectName")]
    project_name: String,

    /// Project status
    status: String,

    /// Project owner ID
    owner: String,

    /// Project description
    description: String,

    /// Region where the project is located
    region: String,

    /// Creation time
    #[serde(rename = "createTime")]
    create_time: String,

    /// Last modification time
    #[serde(rename = "lastModifyTime")]
    last_modify_time: String,

    /// Resource group ID
    #[serde(rename = "resourceGroupId", skip_serializing_if = "Option::is_none")]
    resource_group_id: Option<String>,

    /// Data redundancy type (LRS or ZRS)
    #[serde(rename = "dataRedundancyType")]
    data_redundancy_type: String,
}
