use crate::compress::CompressType;
use crate::response::FromHttpResponse;
use crate::RequestError;

pub(crate) trait Request: Sized + Send + Sync {
    const HTTP_METHOD: http::Method;
    const CONTENT_TYPE: Option<http::HeaderValue> = None;
    const COMPRESS_TYPE: Option<CompressType> = None;
    type ResponseBody: FromHttpResponse + Send + Sync + Sized;
    fn project(&self) -> Option<&str>;
    fn path(&self) -> &str;

    fn query_params(&self) -> Option<Vec<(String, String)>> {
        None
    }

    fn body(&self) -> crate::Result<Option<bytes::Bytes>, RequestError> {
        Ok(None)
    }
    fn headers(&self) -> http::HeaderMap {
        http::HeaderMap::new()
    }
}
