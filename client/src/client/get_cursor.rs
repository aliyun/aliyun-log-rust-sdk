use super::*;
use crate::{RequestErrorKind, ResponseResult};
use serde::Deserialize;

impl crate::client::Client {
    /// Get a cursor for a shard of an existing logstore.
    ///
    /// This method retrieves a cursor that represents a specific position in a shard.
    /// The cursor can be used to read logs from that position using the `pull_logs` method.
    /// You can get a cursor pointing to the beginning of the shard, the end of the shard,
    /// or at a specific time.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore containing the shard
    /// * `shard_id` - The ID of the shard to get a cursor for
    ///
    /// # Examples
    ///
    /// Getting a cursor at different positions in a shard:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_sdk::Client) -> Result<(), aliyun_log_sdk::Error> {
    /// use aliyun_log_sdk::get_cursor_models::CursorPos;
    /// let shard_id = 0;
    /// let resp = client
    ///     .get_cursor("my-project", "my-logstore", shard_id)
    ///     .cursor_pos(CursorPos::Begin) // Get cursor at the beginning of the shard
    ///     .send()
    ///     .await?;
    ///
    /// println!("Begin cursor: {}", resp.get_body().cursor());
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_cursor(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
        shard_id: i32,
    ) -> GetCursorRequestBuilder {
        GetCursorRequestBuilder {
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/shards/{}", logstore.as_ref(), shard_id),
            handle: self.handle.clone(),
            cursor_pos: None,
        }
    }
}

struct GetCursorRequest {
    project: String,
    path: String,
    cursor_pos: get_cursor_models::CursorPos,
}

impl Request for GetCursorRequest {
    type ResponseBody = GetCursorResponse;
    const HTTP_METHOD: http::Method = http::Method::GET;
    fn path(&self) -> &str {
        &self.path
    }
    fn project(&self) -> Option<&str> {
        Some(&self.project)
    }
    fn query_params(&self) -> Option<Vec<(String, String)>> {
        let mut params = Vec::new();
        params.push(("type".to_string(), "cursor".to_string()));
        match &self.cursor_pos {
            get_cursor_models::CursorPos::Begin => {
                params.push(("from".to_string(), "begin".to_string()))
            }
            get_cursor_models::CursorPos::End => {
                params.push(("from".to_string(), "end".to_string()))
            }
            get_cursor_models::CursorPos::UnixTimeStamp(t) => {
                params.push(("from".to_string(), t.to_string()))
            }
        }
        Some(params)
    }
}

pub struct GetCursorRequestBuilder {
    project: String,
    path: String,
    handle: HandleRef,
    cursor_pos: Option<get_cursor_models::CursorPos>,
}

impl GetCursorRequestBuilder {
    /// Required, the cursor position to get.
    pub fn cursor_pos(mut self, cursor_pos: get_cursor_models::CursorPos) -> Self {
        self.cursor_pos = Some(cursor_pos);
        self
    }

    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<GetCursorResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<GetCursorRequest> {
        if self.cursor_pos.is_none() {
            return Err(RequestErrorKind::MissingRequiredParameter(
                "cursor_pos".to_string(),
            ))?;
        }
        Ok((
            self.handle,
            GetCursorRequest {
                cursor_pos: self.cursor_pos.unwrap(),
                project: self.project,
                path: self.path,
            },
        ))
    }
}

pub mod get_cursor_models {
    #[derive(Clone, Default)]
    pub enum CursorPos {
        #[default]
        /// The beginning cursor of the shard.
        Begin,
        /// The end cursor of the shard currently.
        End,
        /// Unix timestamp in seconds, e.g., 1617235200.
        UnixTimeStamp(i64),
    }
}

#[derive(Debug, Default, Deserialize)]
pub struct GetCursorResponse {
    cursor: String,
}

impl GetCursorResponse {
    /// The cursor.
    pub fn cursor(&self) -> &str {
        &self.cursor
    }
}

impl FromHttpResponse for GetCursorResponse {
    fn try_from(body: bytes::Bytes, http_headers: &http::HeaderMap) -> ResponseResult<Self> {
        parse_json_response(body.as_ref(), http_headers)
    }
}
