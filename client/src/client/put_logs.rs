use crate::compress::CompressType;
use crate::error::Result;
use crate::{common::*, RequestError, RequestErrorKind};
use aliyun_log_sdk_protobuf::LogGroup;

use super::*;

impl crate::client::Client {
    /// Write logs to a logstore.
    ///
    /// This method allows sending logs to the specified logstore in an Aliyun Log Service project.
    /// Logs are sent as a LogGroup which can contain multiple individual log entries.
    /// The data is automatically compressed using LZ4 before transmission to optimize bandwidth usage.
    ///
    /// # Arguments
    ///
    /// * `project` - The name of the project containing the logstore
    /// * `logstore` - The name of the logstore to write logs to
    ///
    /// # Examples
    ///
    /// Basic usage with a single log entry:
    ///
    /// ```
    /// # async fn example(client: aliyun_log_sdk::Client) -> Result<(), aliyun_log_sdk::Error> {
    /// use aliyun_log_sdk_protobuf::{Log, LogGroup};
    ///
    /// let mut log = Log::from_unixtime(chrono::Utc::now().timestamp() as u32);
    /// log.add_content_kv("level", "info")
    ///     .add_content_kv("message", "Hello from Rust SDK")
    ///     .add_content_kv("service", "user-service");
    ///
    /// let mut log_group = LogGroup::new();
    /// log_group.add_log(log);
    ///
    /// client.put_logs("my-project", "my-logstore")
    ///     .log_group(log_group)
    ///     .send().await?;
    ///
    /// println!("Logs sent successfully");
    /// # Ok(())
    /// # }
    /// ```
    pub fn put_logs(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
    ) -> PutLogsRequestBuilder {
        PutLogsRequestBuilder {
            handle: self.handle.clone(),
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/shards/lb", logstore.as_ref()),
            log_group: None,
        }
    }
}

pub struct PutLogsRequestBuilder {
    project: String,
    path: String,
    log_group: Option<LogGroup>,
    handle: HandleRef,
}

impl PutLogsRequestBuilder {
    /// The log group to write to the destination logstore, which contains multiple logs.
    pub fn log_group(mut self, log_group: LogGroup) -> Self {
        self.log_group = Some(log_group);
        self
    }

    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<PutLogsResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    fn build(self) -> BuildResult<PutLogsRequest> {
        Ok((
            self.handle,
            PutLogsRequest {
                log_group: self
                    .log_group
                    .ok_or_else(|| {
                        crate::RequestErrorKind::MissingRequiredParameter("log_group".to_string())
                    })
                    .map_err(RequestError::from)?,
                path: self.path,
                project: self.project,
            },
        ))
    }
}

type PutLogsResponse = ();

struct PutLogsRequest {
    project: String,
    path: String,
    log_group: LogGroup,
}

impl Request for PutLogsRequest {
    const HTTP_METHOD: http::Method = http::Method::POST;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_PROTOBUF);
    const COMPRESS_TYPE: Option<CompressType> = Some(CompressType::Lz4);
    type ResponseBody = ();

    fn project(&self) -> Option<&str> {
        Some(self.project.as_str())
    }
    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> Result<Option<bytes::Bytes>, RequestError> {
        let body = self
            .log_group
            .encode()
            .map_err(RequestErrorKind::from)
            .map_err(RequestError::from)?;
        Ok(Some(body.into()))
    }
}
