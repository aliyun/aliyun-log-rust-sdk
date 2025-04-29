use crate::common::LOG_REQUEST_ID;
use crate::utils::ValueGetter;
use crate::ResponseError;

pub struct Response<B = ()>
where
    B: FromHttpResponse + Send + Sync + Sized,
{
    pub(crate) body: B,
    pub(crate) headers: http::HeaderMap,
    pub(crate) status: http::status::StatusCode,
}

impl<B> Response<B>
where
    B: FromHttpResponse + Send + Sync + Sized,
{
    pub fn get_request_id(&self) -> Option<String> {
        self.headers.get_str(LOG_REQUEST_ID)
    }

    pub fn get_headers(&self) -> &http::HeaderMap {
        &self.headers
    }

    pub fn get_body(&self) -> &B {
        &self.body
    }

    pub fn take_body(self) -> B {
        self.body
    }

    pub fn get_http_status(&self) -> &http::StatusCode {
        &self.status
    }
}

#[allow(dead_code)]
pub(crate) struct DecompressedResponse {
    pub(crate) headers: http::HeaderMap,
    pub(crate) status: http::status::StatusCode,
    pub(crate) decompressed: Vec<u8>,
}

pub trait FromHttpResponse: Sized {
    fn try_from(
        bytes: bytes::Bytes,
        headers: &http::HeaderMap,
    ) -> std::result::Result<Self, ResponseError>;
}

impl FromHttpResponse for () {
    fn try_from(_: bytes::Bytes, _: &http::HeaderMap) -> crate::Result<Self, ResponseError> {
        Ok(())
    }
}
