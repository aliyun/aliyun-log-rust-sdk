use http::{HeaderMap, HeaderName, HeaderValue};

use crate::utils::ValueGetter;
use crate::{ResponseErrorKind, ResponseResult};

pub(crate) fn parse_json_response<'a, T>(
    body: &'a [u8],
    headers: &HeaderMap<HeaderValue>,
) -> ResponseResult<T>
where
    T: serde::Deserialize<'a>,
{
    let request_id = headers.get_str(LOG_REQUEST_ID);
    serde_json::from_slice(body)
        .map_err(|source| ResponseErrorKind::JsonDecode { source, request_id }.into())
}

pub(crate) const LOG_REQUEST_ID: HeaderName = HeaderName::from_static("x-log-requestid");
pub(crate) const LOG_BODY_RAW_SIZE: HeaderName = HeaderName::from_static("x-log-bodyrawsize");
pub(crate) const LOG_COMPRESS_TYPE: HeaderName = HeaderName::from_static("x-log-compresstype");
pub(crate) const LOG_PROTOBUF: HeaderValue = HeaderValue::from_static("application/x-protobuf");
pub(crate) const LOG_JSON: HeaderValue = HeaderValue::from_static("application/json");
pub(crate) const LOG_INVALID_COMPRESS_TYPE: HeaderValue =
    HeaderValue::from_static("invalid compress type");
