use super::*;
use crate::{compress::CompressType, error::Result};
use crate::{RequestError, RequestErrorKind, ResponseResult};
use getset::Getters;
use http::header::ACCEPT_ENCODING;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

impl crate::client::Client {
    /// Get logs from a logstore using the given query.
    ///
    /// This method allows you to query logs from a specific logstore within a project.
    /// It supports various query parameters including time range, filtering, and pagination.
    /// The query syntax follows the Aliyun Log Service query language.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore to query logs from
    ///
    /// # Examples
    ///
    /// Basic query with time range, offset, limit, and filter:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_rust_sdk::Client) -> Result<(), aliyun_log_rust_sdk::Error> {
    /// use aliyun_log_rust_sdk::GetLogsRequest;
    /// use chrono::Utc;
    ///
    /// let now = Utc::now().timestamp();
    /// let one_hour_ago = now - 3600;
    /// let resp = client.get_logs("my-project", "my-logstore")
    ///     .from(one_hour_ago)         // Start time (required)
    ///     .to(now)                    // End time (required)
    ///     .query("level:ERROR")       // Filter for error logs only
    ///     .offset(0)                  // Start from the first log
    ///     .line(100)                  // Return up to 100 logs
    ///     .send()
    ///     .await?;
    ///
    /// // Check if the query completed successfully
    /// if resp.get_body().is_complete() {
    ///     println!("Query completed successfully");
    /// } else {
    ///     println!("Query is incomplete, you may need to retry later");
    /// }
    ///
    /// // Process the returned logs
    /// println!("Retrieved {} logs", resp.get_body().logs_count());
    ///
    /// for log in resp.get_body().logs() {
    ///     // Each log is a HashMap<String, String>, print all fields in the log
    ///     for (key, value) in log {
    ///         println!("  {}: {}", key, value);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_logs(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
    ) -> GetLogsRequestBuilder {
        GetLogsRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/logs", logstore.as_ref()),
            handle: self.handle.clone(),
            from: None,
            to: None,
            topic: None,
            line: None,
            offset: None,
            reverse: None,
            query: None,
            power_sql: None,
            need_highlight: None,
            from_ns_part: None,
            to_ns_part: None,
        }
    }
}

#[derive(Serialize)]
pub struct GetLogsRequest {
    #[serde(skip_serializing)]
    project: String,
    #[serde(skip_serializing)]
    path: String,

    from: i64,
    to: i64,

    #[serde(rename = "fromNs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    from_ns_part: Option<u32>,

    #[serde(rename = "toNs")]
    #[serde(skip_serializing_if = "Option::is_none")]
    to_ns_part: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    offset: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    reverse: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<String>,

    #[serde(rename = "powerSql")]
    #[serde(skip_serializing_if = "Option::is_none")]
    power_sql: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    need_highlight: Option<bool>,
}

impl Request for GetLogsRequest {
    const HTTP_METHOD: http::Method = http::Method::POST;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_JSON);
    type ResponseBody = GetLogsResponse;
    fn project(&self) -> Option<&str> {
        Some(self.project.as_str())
    }
    fn path(&self) -> &str {
        &self.path
    }
    fn body(&self) -> Result<Option<bytes::Bytes>, RequestError> {
        let body = serde_json::to_string(self)
            .map(|s| s.into_bytes())
            .map_err(RequestErrorKind::from)
            .map_err(RequestError::from)?;
        Ok(Some(body.into()))
    }
    fn headers(&self) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            ACCEPT_ENCODING,
            CompressType::Lz4
                .to_string()
                .parse()
                .expect("fail to insert CompressType into headers"),
        );
        headers
    }
}

pub struct GetLogsRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,

    from: Option<i64>,
    to: Option<i64>,
    topic: Option<String>,
    line: Option<u32>,
    offset: Option<u32>,
    reverse: Option<bool>,
    query: Option<String>,
    power_sql: Option<bool>,
    from_ns_part: Option<u32>,
    to_ns_part: Option<u32>,
    need_highlight: Option<bool>,
}

impl GetLogsRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<GetLogsResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }
    /// Required, the start time of the query, in unix timestamp, in seconds, e.g., 1609459200.
    pub fn from(mut self, from: i64) -> Self {
        self.from = Some(from);
        self
    }

    /// Required, the end time of the query, in unix timestamp, in seconds, e.g., 1609459200.
    pub fn to(mut self, to: i64) -> Self {
        self.to = Some(to);
        self
    }

    /// Optional, the topic of the logs to query.
    pub fn topic<T: Into<String>>(mut self, topic: T) -> Self {
        self.topic = Some(topic.into());
        self
    }

    /// Optional, the maximum number of logs to return for a non-SQL query.
    /// If omitted, the service defaults to 100. For SQL queries, use a LIMIT clause.
    pub fn line(mut self, line: u32) -> Self {
        self.line = Some(line);
        self
    }

    /// Alias for [`Self::line`], retained for compatibility with existing callers.
    pub fn lines(self, lines: u32) -> Self {
        self.line(lines)
    }

    /// Optional, the starting row for a non-SQL query, default 0.
    /// For SQL queries, use a LIMIT clause instead.
    pub fn offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }

    /// Optional, whether to return the logs in reverse order, default false.
    pub fn reverse(mut self, reverse: bool) -> Self {
        self.reverse = Some(reverse);
        self
    }

    /// Required, the query string to use.
    pub fn query<T: Into<String>>(mut self, query: T) -> Self {
        self.query = Some(query.into());
        self
    }

    /// Optional, whether to use power SQL.
    pub fn power_sql(mut self, power_sql: bool) -> Self {
        self.power_sql = Some(power_sql);
        self
    }

    /// Optional, the nano part of start time of the query, ranges from 0 to 999999999.
    pub fn from_ns_part(mut self, from_ns_part: u32) -> Self {
        self.from_ns_part = Some(from_ns_part);
        self
    }

    /// Optional, the nano part of end time of the query, ranges from 0 to 999999999.
    pub fn to_ns_part(mut self, to_ns_part: u32) -> Self {
        self.to_ns_part = Some(to_ns_part);
        self
    }

    /// Optional, whether to return the highlight of query results.
    pub fn need_highlight(mut self, need_highlight: bool) -> Self {
        self.need_highlight = Some(need_highlight);
        self
    }

    fn build(self) -> BuildResult<GetLogsRequest> {
        check_required!(("from", self.from), ("to", self.to));

        Ok((
            self.handle,
            GetLogsRequest {
                from: self.from.unwrap(),
                to: self.to.unwrap(),
                topic: self.topic,
                line: self.line,
                offset: self.offset,
                reverse: self.reverse,
                query: self.query,
                power_sql: self.power_sql,
                from_ns_part: self.from_ns_part,
                to_ns_part: self.to_ns_part,
                need_highlight: self.need_highlight,
                project: self.project,
                path: self.path,
            },
        ))
    }
}

#[derive(Debug, Deserialize)]
pub struct GetLogsResponse {
    meta: get_logs_models::GetLogsMeta,
    #[serde(rename = "data")]
    logs: Vec<HashMap<String, String>>,
}

impl GetLogsResponse {
    /// Returns true if the query is complete.
    pub fn is_complete(&self) -> bool {
        self.meta().progress().eq_ignore_ascii_case("complete")
    }
    /// Returns the number of logs returned.
    pub fn logs_count(&self) -> usize {
        self.logs.len()
    }
    /// Takes out the logs from the response.
    pub fn take_logs(self) -> Vec<HashMap<String, String>> {
        self.logs
    }
    /// Returns the queried logs.
    pub fn logs(&self) -> &Vec<HashMap<String, String>> {
        &self.logs
    }
    /// Returns the queried logs as mutable.
    pub fn logs_mut(&mut self) -> &mut Vec<HashMap<String, String>> {
        &mut self.logs
    }
    pub fn meta(&self) -> &get_logs_models::GetLogsMeta {
        &self.meta
    }
}

impl FromHttpResponse for GetLogsResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        parse_json_response(body.as_ref(), http_headers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> Client {
        let config = Config::builder()
            .endpoint("localhost")
            .access_key("test-id", "test-secret")
            .build()
            .unwrap();
        Client::from_config(config).unwrap()
    }

    #[test]
    fn request_body_uses_line_for_explicit_limits() {
        let client = client();
        for lines in [0, 1, 17, 100] {
            let (_, request) = client
                .get_logs("test-project", "test-logstore")
                .from(1609459200)
                .to(1609462800)
                .query("V2")
                .offset(0)
                .line(lines)
                .reverse(false)
                .build()
                .unwrap();
            let body = request.body().unwrap().unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["line"], lines);
            assert!(body.get("lines").is_none());
            assert_eq!(body["offset"], 0);
            assert_eq!(body["query"], "V2");
            assert_eq!(body["reverse"], false);
        }
    }

    #[test]
    fn lines_alias_preserves_limits_and_last_setter_wins() {
        let client = client();
        let builders = [
            client.get_logs("test-project", "test-logstore").lines(1),
            client
                .get_logs("test-project", "test-logstore")
                .line(17)
                .lines(1),
            client
                .get_logs("test-project", "test-logstore")
                .lines(17)
                .line(1),
        ];
        for builder in builders {
            let (_, request) = builder.from(1609459200).to(1609462800).build().unwrap();
            let body = request.body().unwrap().unwrap();
            let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(body["line"], 1);
            assert!(body.get("lines").is_none());
        }
    }

    #[test]
    fn request_body_omits_unspecified_line_limit() {
        let (_, request) = client()
            .get_logs("test-project", "test-logstore")
            .from(1609459200)
            .to(1609462800)
            .query("*")
            .build()
            .unwrap();
        let body = request.body().unwrap().unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(body.get("line").is_none());
        assert!(body.get("lines").is_none());
    }
}

pub mod get_logs_models {
    use super::*;
    #[derive(Debug, Deserialize, Default, Getters)]
    #[serde(rename_all = "snake_case", default = "GetLogsMeta::default")]
    #[allow(dead_code)]
    #[getset(get = "pub")]
    pub struct GetLogsMeta {
        progress: String,
        agg_query: Option<String>,
        where_query: Option<String>,
        #[serde(rename = "hasSQL")]
        has_sql: Option<bool>,
        processed_rows: Option<i64>,
        elapsed_millisecond: Option<i64>,
        cpu_sec: Option<f64>,
        cpu_cores: Option<f64>,
        limited: Option<i64>,
        count: Option<i64>,
        processed_bytes: Option<i64>,
        telementry_type: Option<String>,
        power_sql: Option<bool>,
        #[serde(rename = "insertedSQL")]
        inserted_sql: Option<String>,
        keys: Option<Vec<String>>,
        terms: Option<Vec<MetaTerm>>,
        marker: Option<String>,
        mode: Option<i32>,
        phrase_query_info: Option<PhraseQueryInfoV3>,
        shard: Option<i32>,
        scan_bytes: Option<i64>,
        is_accurate: Option<bool>,
        column_types: Option<Vec<String>>,
        highlights: Option<Vec<HashMap<String, String>>>,
    }

    #[derive(Debug, Deserialize, Getters)]
    #[allow(dead_code)]
    #[getset(get = "pub")]
    pub struct MetaTerm {
        key: String,
        term: String,
    }

    #[derive(Debug, Deserialize, Getters)]
    #[allow(dead_code)]
    #[getset(get = "pub")]
    pub struct PhraseQueryInfoV3 {
        scan_all: Option<bool>,
        begin_offset: Option<i64>,
        end_offset: Option<i64>,
        end_time: Option<i64>,
    }
}
