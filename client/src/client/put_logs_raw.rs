use crate::error::Result;
use crate::{common::*, RequestError};

use super::*;

impl crate::client::Client {
    #[doc(hidden)]
    pub fn put_logs_raw(
        &self,
        project: impl AsRef<str>,
        logstore: impl AsRef<str>,
    ) -> PutLogsRawRequestBuilder {
        PutLogsRawRequestBuilder {
            handle: self.handle.clone(),
            project: project.as_ref().to_string(),
            path: format!("/logstores/{}/shards/lb", logstore.as_ref()),
            data: None,
            raw_size: None,
            compress_type: None,
        }
    }
}

pub struct PutLogsRawRequestBuilder {
    project: String,
    path: String,
    data: Option<bytes::Bytes>,
    raw_size: Option<usize>,
    compress_type: Option<String>,
    handle: HandleRef,
}

impl PutLogsRawRequestBuilder {
    #[must_use = "the result future must be awaited"]
    pub fn send(self) -> ResponseResultBoxFuture<PutLogsRawResponse> {
        Box::pin(async move {
            let (handle, request) = self.build()?;
            handle.send(request).await
        })
    }

    #[doc(hidden)]
    pub fn data(mut self, data: bytes::Bytes) -> Self {
        self.data = Some(data);
        self
    }

    #[doc(hidden)]
    pub fn raw_size(mut self, raw_size: usize) -> Self {
        self.raw_size = Some(raw_size);
        self
    }

    #[doc(hidden)]
    pub fn compress_type(mut self, compress_type: String) -> Self {
        self.compress_type = Some(compress_type);
        self
    }

    fn build(self) -> BuildResult<PutLogsRawRequest> {
        Ok((
            self.handle,
            PutLogsRawRequest {
                path: self.path,
                project: self.project,
                data: require_param("data", self.data)?,
                raw_size: require_param("raw_size", self.raw_size)?,
                compress_type: require_param("compress_type", self.compress_type)?,
            },
        ))
    }
}

type PutLogsRawResponse = ();

struct PutLogsRawRequest {
    project: String,
    path: String,
    data: bytes::Bytes,
    raw_size: usize,
    compress_type: String,
}

impl Request for PutLogsRawRequest {
    const HTTP_METHOD: http::Method = http::Method::POST;
    const CONTENT_TYPE: Option<http::HeaderValue> = Some(LOG_PROTOBUF);
    type ResponseBody = ();

    fn project(&self) -> Option<&str> {
        Some(self.project.as_str())
    }
    fn path(&self) -> &str {
        &self.path
    }

    fn body(&self) -> Result<Option<bytes::Bytes>, RequestError> {
        Ok(Some(self.data.clone()))
    }

    fn headers(&self) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert(
            LOG_BODY_RAW_SIZE,
            self.raw_size.to_string().parse().unwrap(),
        );
        headers.insert(
            LOG_COMPRESS_TYPE,
            self.compress_type
                .parse()
                .unwrap_or(LOG_INVALID_COMPRESS_TYPE),
        );
        headers
    }
}
