use super::*;
use super::{BuildResult, HandleRef};
use crate::compress::CompressType;
use crate::request::Request;
use crate::response::FromHttpResponse;
use crate::utils::ValueGetter;
use crate::{ResponseError, ResponseErrorKind, ResponseResult};
use aliyun_log_sdk_protobuf::{LogGroup, LogGroupList};
use getset::Getters;
use http::header::{ACCEPT, ACCEPT_ENCODING};

impl crate::client::Client {
    /// Pull logs from a shard of a logstore from the given cursor.
    ///
    /// This method allows retrieving logs from a specific shard within a logstore,
    /// starting from a specified cursor position. It supports filtering logs with a query,
    /// limiting the number of logs retrieved, and controlling the log range with
    /// start and end cursors.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore to pull logs from
    /// * `shard_id` - The ID of the shard to pull logs from
    ///
    /// # Examples
    ///
    /// ## Basic usage:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// use aliyun_log_rust_sdk::get_cursor_models::CursorPos;
    /// let shard_id = 0;
    ///
    /// // First, get a cursor for a shard
    /// let resp = client.get_cursor("my-project", "my-logstore", shard_id)
    ///     .cursor_pos(CursorPos::Begin)
    ///     .send().await?;
    /// let cursor = resp.get_body().cursor();
    ///
    /// // Then, pull logs using that cursor
    /// let resp = client.pull_logs("my-project", "my-logstore", shard_id)
    ///     .cursor(cursor)
    ///     .count(100)  // Returns up to 100 log groups
    ///     .query("* | where level = 'ERROR'")  // Optional: filter logs with a SPL query
    ///     .send().await?;
    ///
    /// // Print the pulled logs
    /// for log_group in resp.get_body().log_group_list() {
    ///     for log in log_group.logs() {
    ///         println!("Log time: {}", log.time());
    ///         for content in log.contents() {
    ///             println!("  {}: {}", content.key(), content.value());
    ///         }
    ///     }
    /// }
    ///
    /// // To continue pulling logs, use the next cursor
    /// let next_cursor = resp.get_body().next_cursor();
    /// println!("{}", next_cursor);
    /// # Ok(())
    /// # }
    /// ```
    ///
    ///
    /// ## Advanced usage with cursor range:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// use aliyun_log_rust_sdk::get_cursor_models::CursorPos;
    /// let shard_id = 0; // Shard ID to pull logs from
    /// // Get start and end cursors (for a time range)
    /// let resp = client.get_cursor("my-project", "my-logstore", shard_id)
    ///     .cursor_pos(CursorPos::UnixTimeStamp(1700000000))
    ///     .send().await?;
    /// let start_cursor = resp.get_body().cursor();
    ///
    /// let resp = client.get_cursor("my-project", "my-logstore", shard_id)
    ///     .cursor_pos(CursorPos::UnixTimeStamp(1700001234))
    ///     .send().await?;
    /// let end_cursor = resp.get_body().cursor();
    ///
    /// // Pull logs between [begin_cursor, end_cursor)
    /// let resp = client.pull_logs("my-project", "my-logstore", shard_id)
    ///     .cursor(start_cursor)
    ///     .end_cursor(end_cursor)
    ///     .count(1000)
    ///     .send().await?;
    ///
    /// println!("Retrieved {} log groups", resp.get_body().log_group_count());
    /// # Ok(())
    /// # }
    /// ```
    pub fn pull_logs(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        shard_id: i32,
    ) -> PullLogsRequestBuilder {
        PullLogsRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/shards/{}", logstore.as_ref(), shard_id),
            handle: self.handle.clone(),
            cursor: None,
            end_cursor: None,
            count: None,
            query: None,
            query_id: None,
        }
    }
}

pub struct PullLogsRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    cursor: Option<String>,
    end_cursor: Option<String>,
    count: Option<i32>,
    query: Option<String>,
    query_id: Option<String>,
}

impl PullLogsRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<PullLogsResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    /// Required, the cursor to start pulling logs from, inclusive.
    pub fn cursor<T: Into<String>>(mut self, cursor: T) -> Self {
        self.cursor = Some(cursor.into());
        self
    }

    /// Optional, the cursor to end pulling logs, exclusive.
    pub fn end_cursor<T: Into<String>>(mut self, end_cursor: T) -> Self {
        self.end_cursor = Some(end_cursor.into());
        self
    }

    /// Required, the maximum number of log groups to pull.
    pub fn count(mut self, count: i32) -> Self {
        self.count = Some(count);
        self
    }

    /// Optional, the query to filter logs, using the spl syntax, e.g, "* | where name = 'Mike'".
    pub fn query<T: Into<String>>(mut self, query: T) -> Self {
        self.query = Some(query.into());
        self
    }

    pub fn query_id<T: Into<String>>(mut self, query_id: T) -> Self {
        self.query_id = Some(query_id.into());
        self
    }

    fn build(self) -> BuildResult<PullLogsRequest> {
        check_required!(("cursor", self.cursor), ("count", self.count));

        Ok((
            self.handle.clone(),
            PullLogsRequest {
                cursor: self.cursor.unwrap(),
                end_cursor: self.end_cursor,
                count: self.count.unwrap(),
                query: self.query,
                query_id: self.query_id,
                project: self.project,
                path: self.path,
            },
        ))
    }
}

#[derive(Debug, Getters, Default)]
pub struct PullLogsResponse {
    #[getset(get = "pub", get_mut = "pub")]
    log_group_list: Vec<LogGroup>,
    #[getset(get = "pub")]
    next_cursor: String,
    #[getset(get = "pub")]
    log_group_count: i32,
    #[getset(get = "pub")]
    read_last_cursor: Option<String>,
    #[getset(get = "pub")]
    raw_size_before_query: Option<i32>,
    #[getset(get = "pub")]
    data_count_before_query: Option<i32>,
    #[getset(get = "pub")]
    result_lines: Option<i32>,
    #[getset(get = "pub")]
    lines_before_query: Option<i32>,
    #[getset(get = "pub")]
    failed_lines: Option<i32>,
}

impl PullLogsResponse {
    pub fn into_log_group_list(self) -> Vec<LogGroup> {
        self.log_group_list
    }
}

impl FromHttpResponse for PullLogsResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        let request_id = http_headers.get_str(LOG_REQUEST_ID);
        let log_group_list: LogGroupList = LogGroupList::decode(body.as_ref())
            .map_err(|source| ResponseErrorKind::ProtobufDeserialize { source, request_id })
            .map_err(ResponseError::from)?;

        let log_group_count = http_headers.get_i32_or_default("x-log-count", 0);
        let read_last_cursor = http_headers.get_str("x-log-read-last-cursor");
        let raw_size_before_query = http_headers.get_i32("x-log-rawdatasize");
        let data_count_before_query = http_headers.get_i32("x-log-rawdatacount");
        let result_lines = http_headers.get_i32("x-log-resultlines");
        let lines_before_query = http_headers.get_i32("x-log-rawdatalines");
        let failed_lines = http_headers.get_i32("x-log-failedlines");
        let next_cursor = http_headers.get_str_or_default("x-log-cursor", "");

        Ok(PullLogsResponse {
            log_group_list: log_group_list.into(),
            next_cursor,
            log_group_count,
            read_last_cursor,
            data_count_before_query,
            result_lines,
            lines_before_query,
            failed_lines,
            raw_size_before_query,
        })
    }
}

struct PullLogsRequest {
    project: String,
    path: String,
    cursor: String,
    end_cursor: Option<String>,
    count: i32,
    query: Option<String>,
    query_id: Option<String>,
}

impl Request for PullLogsRequest {
    const HTTP_METHOD: http::Method = http::Method::GET;
    type ResponseBody = PullLogsResponse;

    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }

    fn path(&self) -> &str {
        &self.path
    }
    fn headers(&self) -> http::HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, LOG_PROTOBUF);
        headers.insert(
            ACCEPT_ENCODING,
            CompressType::Lz4.to_string().parse().expect("fail to insert CompressType into headers"),
        );
        headers
    }
    fn query_params(&self) -> Option<Vec<(String, String)>> {
        let mut params = Vec::new();
        params.push(("type".to_string(), "logs".to_string()));
        params.push(("cursor".to_string(), self.cursor.clone()));
        params.push(("count".to_string(), self.count.to_string()));
        if let Some(end_cursor) = &self.end_cursor {
            params.push(("endCursor".to_string(), end_cursor.clone()));
        }
        if let Some(query) = &self.query {
            params.push(("query".to_string(), query.clone()));
        }
        if let Some(query_id) = &self.query_id {
            params.push(("queryId".to_string(), query_id.clone()));
        }
        Some(params)
    }
}
