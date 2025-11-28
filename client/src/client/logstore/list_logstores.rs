use super::*;
use crate::ResponseResult;
use getset::Getters;
use serde::Deserialize;

impl crate::client::Client {
    /// List logstores with pagination and filtering.
    ///
    /// This method retrieves a list of logstores in a project with support for pagination
    /// and filtering by name, telemetry type, and mode.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project
    /// * `offset` - The offset for pagination (starting from 0)
    /// * `size` - The number of logstores to return (page size)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// // List first 10 logstores
    /// let resp = client.list_logstores("my-project", 0, 10)
    ///     .send()
    ///     .await?;
    ///
    /// println!("Total logstores: {}", resp.get_body().total());
    /// for logstore in resp.get_body().logstores() {
    ///     println!("Logstore: {}", logstore);
    /// }
    ///
    /// // Filter by logstore name
    /// let resp = client.list_logstores("my-project", 0, 10)
    ///     .logstore_name("test")
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn list_logstores(
        &self,
        project: impl AsRef<str>,
        offset: i32,
        size: i32,
    ) -> ListLogstoresRequestBuilder {
        ListLogstoresRequestBuilder {
            project: project.as_ref().to_string(),
            handle: self.handle.clone(),
            offset,
            size,
            logstore_name: None,
            telemetry_type: None,
            mode: None,
        }
    }
}

pub struct ListLogstoresRequestBuilder {
    project: String,
    handle: HandleRef,
    offset: i32,
    size: i32,
    logstore_name: Option<String>,
    telemetry_type: Option<String>,
    mode: Option<String>,
}

impl ListLogstoresRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<ListLogstoresResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Filter logstores by name (fuzzy search).
    ///
    /// # Arguments
    ///
    /// * `logstore_name` - Logstore name to search for (supports partial matching)
    pub fn logstore_name(mut self, logstore_name: impl Into<String>) -> Self {
        self.logstore_name = Some(logstore_name.into());
        self
    }

    /// Filter logstores by telemetry type.
    ///
    /// # Arguments
    ///
    /// * `telemetry_type` - Telemetry type. Valid values:
    ///   - `None`: Query all telemetry types
    ///   - `Metrics`: Query Metrics type only
    pub fn telemetry_type(mut self, telemetry_type: impl Into<String>) -> Self {
        self.telemetry_type = Some(telemetry_type.into());
        self
    }

    /// Filter logstores by mode.
    ///
    /// # Arguments
    ///
    /// * `mode` - Logstore mode. Valid values:
    ///   - `standard`: Standard mode with full query and analysis features
    ///   - `query`: Query mode with high-performance queries but no SQL analysis
    pub fn mode(mut self, mode: impl Into<String>) -> Self {
        self.mode = Some(mode.into());
        self
    }

    fn build(self) -> BuildResult<ListLogstoresRequest> {
        Ok((
            self.handle,
            ListLogstoresRequest {
                project: self.project,
                offset: self.offset,
                size: self.size,
                logstore_name: self.logstore_name,
                telemetry_type: self.telemetry_type,
                mode: self.mode,
            },
        ))
    }
}

struct ListLogstoresRequest {
    project: String,
    offset: i32,
    size: i32,
    logstore_name: Option<String>,
    telemetry_type: Option<String>,
    mode: Option<String>,
}

impl Request for ListLogstoresRequest {
    type ResponseBody = ListLogstoresResponse;
    const HTTP_METHOD: http::Method = http::Method::GET;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        "/logstores"
    }

    fn query_params(&self) -> Option<Vec<(String, String)>> {
        let mut params = vec![
            ("offset".to_string(), self.offset.to_string()),
            ("size".to_string(), self.size.to_string()),
        ];

        if let Some(ref logstore_name) = self.logstore_name {
            params.push(("logstoreName".to_string(), logstore_name.clone()));
        }

        if let Some(ref telemetry_type) = self.telemetry_type {
            params.push(("telemetryType".to_string(), telemetry_type.clone()));
        }

        if let Some(ref mode) = self.mode {
            params.push(("mode".to_string(), mode.clone()));
        }

        Some(params)
    }
}

/// Response containing a list of logstores
#[derive(Debug, Getters, Deserialize)]
#[getset(get = "pub")]
pub struct ListLogstoresResponse {
    /// Number of logstores returned in this response
    count: i32,

    /// Total number of logstores matching the filter criteria
    total: i32,

    /// List of logstore names
    logstores: Vec<String>,
}

impl FromHttpResponse for ListLogstoresResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        parse_json_response(body.as_ref(), http_headers)
    }
}
