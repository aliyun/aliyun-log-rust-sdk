use std::borrow::Cow;

use base64::{engine::general_purpose, Engine as _};
#[cfg(not(test))]
use chrono::Utc;
use http::{
    header::{InvalidHeaderValue, ToStrError, CONTENT_LENGTH, CONTENT_TYPE, DATE},
    HeaderMap, HeaderName, HeaderValue, Method,
};

/// Calculate the signature of HTTP requeust to aliyun log service, using signature version 1.
/// This function modifies the `headers` in place, and should be called just before sending the request.
///
/// # Arguments
///
/// * `access_key_id` - The access key id of your aliyun account.
/// * `access_key_secret` - The access key secret of your aliyun account.
/// * `security_token` - The security token of your aliyun account, which is optional.
/// * `method` - The HTTP method of the request.
/// * `path` - The HTTP path of the request, eg: `/logstores/test_logstore/shards/0`.
/// * `headers` - The HTTP headers of the request.
/// * `query_params` - The HTTP query params of the request, which is optional, eg: `[("key", "value"), ("key2", "value2")].into()`.
/// * `body` - The HTTP body of the request, which is optional.
///
/// # Returns
///
/// A `Result` which is:
///
/// * `Ok(String)` containing the signature of the request, which has already been added to `headers`, so you don't need to add it again.
///   The returned result can be used for testing or logging.
/// * `Err(Error)` if the calculation failed.
///
/// # Errors
///
/// This function will return an error if the calculation failed, the reason can be one of the following:
///
/// * `access_key_id` contains invalid invisible characters which can not be used in HTTP headers.
/// * `security_token` contains invalid invisible characters which can not be used in HTTP headers.
/// * `headers` contains invalid invisible characters, which is not permitted in HTTP headers.
///
/// # Examples
///
/// ```
/// use aliyun_log_sdk_sign::{sign_v1, QueryParams};
/// let mut headers = http::HeaderMap::new();
/// let signature_result = sign_v1(
///     "your_access_key_id",
///     "your_access_key_secret",
///     None,
///     http::Method::GET,
///     "/",
///     &mut headers,
///     QueryParams::empty(),
///     None,
/// );
/// if let Err(err) = signature_result {
///     println!("signature error: {}", err);
/// }
///
/// let signature_result = sign_v1(
///     "your_access_key_id",
///     "your_access_key_secret",
///     Some("your_security_token"),
///     http::Method::POST,
///     "/logstores/test-logstore/logs",
///     &mut headers,
///     [("key", "value"), ("key2", "value2")].into(),
///     Some(b"HTTP body contents"),
/// );
/// if let Err(err) = signature_result {
///     println!("signature error: {}", err);
/// }
/// ```
#[allow(clippy::too_many_arguments)]
pub fn sign_v1(
    access_key_id: &str,
    access_key_secret: &str,
    security_token: Option<&str>,
    method: Method,
    path: &str,
    headers: &mut HeaderMap,
    query_params: QueryParams,
    body: Option<&[u8]>,
) -> Result<String> {
    headers
        .entry(LOG_API_VERSION)
        .or_insert(LOG_API_VERSION_0_6_0);
    headers.insert(LOG_SIGNATURE_METHOD, LOG_SIGNATURE_METHOD_HMAC_SHA1);

    if let Some(security_token) = security_token {
        headers.insert(
            LOG_ACS_SECURITY_TOKEN,
            HeaderValue::from_str(security_token)?,
        );
    }

    let (content_md5, content_len) = calc_md5(body);
    if content_len > 0 {
        headers.insert(
            LOG_CONTENT_MD5,
            HeaderValue::from_str(&content_md5).expect("md5 should be valid in HTTP header"),
        );
    }
    headers.insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&content_len.to_string())
            .expect("content_len should be valid in HTTP header"),
    );

    // date
    let date = now_rfc1123();
    headers.insert(
        DATE,
        HeaderValue::from_str(&date).expect("date should be valid in HTTP header"),
    );

    let content_type = get_content_type(headers)?;
    let mut builder = string_builder::Builder::default();
    builder.append(format!(
        "{}\n{}\n{}\n{}\n",
        method, content_md5, content_type, date
    ));

    // headers
    let mut sorted_header: Vec<_> = headers.iter().collect();
    sorted_header.sort_by_key(|x| x.0.as_str());

    for (k, v) in sorted_header {
        let k = k.as_str();
        if !k.starts_with("x-log-") && !k.starts_with("x-acs-") {
            continue;
        }
        if let Ok(v) = v.to_str() {
            builder.append(k);
            builder.append(":");
            builder.append(v);
            builder.append("\n");
        }
    }

    // url & params
    builder.append(path);
    let mut query_pairs = query_params.clone();

    if !query_pairs.0.is_empty() {
        builder.append("?");
        query_pairs.0.sort_by_key(|x| x.0.clone());
        let mut sep = "";
        for (k, v) in query_pairs.0.iter() {
            builder.append(sep);
            builder.append(k.as_bytes());
            builder.append("=");
            builder.append(v.as_bytes());
            sep = "&";
        }
    }
    let message = builder
        .string()
        .expect("fail to build message, invalid utf8");
    trace!("signature message: {}", message);

    let signature = general_purpose::STANDARD.encode(hmac_sha1::hmac_sha1(
        access_key_secret.as_bytes(),
        message.as_bytes(),
    ));
    let auth = format!("LOG {access_key_id}:{signature}");
    headers.insert(LOG_AUTHORIZATION, HeaderValue::from_str(&auth)?);
    Ok(auth)
}

#[derive(Debug, Clone, Default)]
pub struct QueryParams<'a>(Vec<(Cow<'a, str>, Cow<'a, str>)>);

impl QueryParams<'_> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn empty() -> Self {
        Self::default()
    }
}

impl<'a, K, V, I> From<I> for QueryParams<'a>
where
    K: Into<Cow<'a, str>>,
    V: Into<Cow<'a, str>>,
    I: IntoIterator<Item = (K, V)>,
{
    fn from(iter: I) -> Self {
        Self(
            iter.into_iter()
                .map(|(k, v)| (k.into(), v.into()))
                .collect(),
        )
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),

    #[error("invalid header value to str: {0}")]
    ToStrError(#[from] ToStrError),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

fn calc_md5(body: Option<&[u8]>) -> (String, usize) {
    if let Some(content) = body {
        let len = content.len();
        if len > 0 {
            let content_md5 = format!("{:X}", md5::compute(content));
            return (content_md5, len);
        }
    }
    (String::default(), 0)
}

fn get_content_type(headers: &HeaderMap) -> Result<String> {
    if let Some(content_type) = headers.get(CONTENT_TYPE) {
        Ok(content_type.to_str()?.to_owned())
    } else {
        Ok(String::default())
    }
}

#[cfg(not(test))]
fn now_rfc1123() -> String {
    Utc::now().format("%a, %d %b %Y %H:%M:%S GMT").to_string()
}

#[cfg(test)]
const TEST_NOW_RFC1123: &str = "Thu, 01 Jan 1970 00:00:00 GMT";

#[cfg(test)]
fn now_rfc1123() -> String {
    String::from(TEST_NOW_RFC1123)
}

const LOG_API_VERSION: HeaderName = HeaderName::from_static("x-log-apiversion");
const LOG_SIGNATURE_METHOD: HeaderName = HeaderName::from_static("x-log-signaturemethod");
const LOG_CONTENT_MD5: HeaderName = HeaderName::from_static("content-md5");
const LOG_AUTHORIZATION: HeaderName = HeaderName::from_static("authorization");
const LOG_ACS_SECURITY_TOKEN: HeaderName = HeaderName::from_static("x-acs-security-token");
const LOG_API_VERSION_0_6_0: HeaderValue = HeaderValue::from_static("0.6.0");
const LOG_SIGNATURE_METHOD_HMAC_SHA1: HeaderValue = HeaderValue::from_static("hmac-sha1");

#[allow(dead_code)]
#[non_exhaustive]
enum SignatureVersion {
    V1,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Trace)
            .try_init();
    }

    #[test]
    fn test_sign_case1() {
        init();

        let mut headers = HeaderMap::new();
        let signature = sign_v1(
            "",
            "",
            None,
            Method::GET,
            "/",
            &mut headers,
            QueryParams::empty(),
            None,
        )
        .unwrap();
        assert_eq!(signature, "LOG :SApFTtfTFKHmzdEdaMe5TjNn+RQ=");
        assert!(headers.contains_key(LOG_AUTHORIZATION));
        assert!(headers.contains_key(DATE));
        assert!(headers.contains_key(LOG_API_VERSION));
        assert!(headers.contains_key(LOG_SIGNATURE_METHOD));
        assert!(headers.contains_key(CONTENT_LENGTH));
        assert!(!headers.contains_key(LOG_CONTENT_MD5));
        assert!(!headers.contains_key(LOG_ACS_SECURITY_TOKEN));
    }

    #[test]
    fn test_sign_case2() {
        init();

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let body = r#"
            {"key": "value"}
        "#;

        let signature = sign_v1(
            "test-access-key-id",
            "test-access-key",
            None,
            Method::POST,
            "/logstores/test-logstore",
            &mut headers,
            [("type", "log"), ("offset", "0"), ("line", "100")].into(),
            Some(body.as_bytes()),
        )
        .unwrap();
        assert_eq!(
            signature,
            "LOG test-access-key-id:4pL2xZJERC3tPKtRiHh9+nMG3tI="
        );
        assert!(headers.contains_key(LOG_AUTHORIZATION));
        assert!(headers.contains_key(DATE));
        assert!(headers.contains_key(LOG_API_VERSION));
        assert!(headers.contains_key(LOG_SIGNATURE_METHOD));
        assert!(headers.contains_key(CONTENT_LENGTH));
        assert!(headers.contains_key(LOG_CONTENT_MD5));
        assert!(!headers.contains_key(LOG_ACS_SECURITY_TOKEN));
        assert_eq!(
            "CE688F8D1AC3ED309BA9BE0A5ABAFCE5",
            headers.get(LOG_CONTENT_MD5).unwrap().to_str().unwrap()
        );
    }

    #[test]
    fn test_sign_case3() {
        init();

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let body = r#"
            {"key": "value"}
        "#;

        let signature = sign_v1(
            "test-access-key-id",
            "test-access-key",
            Some("test-security-token"),
            Method::POST,
            "/logstores/test-logstore",
            &mut headers,
            [("type", "log"), ("offset", "0"), ("line", "100")].into(),
            Some(body.as_bytes()),
        )
        .unwrap();
        assert_eq!(
            signature,
            "LOG test-access-key-id:ZQt+0wIvpd+O9yIJKeKxZTJ2hv0="
        );
        assert!(headers.contains_key(LOG_AUTHORIZATION));
        assert!(headers.contains_key(DATE));
        assert!(headers.contains_key(LOG_API_VERSION));
        assert!(headers.contains_key(LOG_SIGNATURE_METHOD));
        assert!(headers.contains_key(CONTENT_LENGTH));
        assert!(headers.contains_key(LOG_CONTENT_MD5));
        assert!(headers.contains_key(LOG_ACS_SECURITY_TOKEN));
        assert_eq!(
            "CE688F8D1AC3ED309BA9BE0A5ABAFCE5",
            headers.get(LOG_CONTENT_MD5).unwrap().to_str().unwrap()
        );
        assert_eq!(
            "test-security-token",
            headers
                .get(LOG_ACS_SECURITY_TOKEN)
                .unwrap()
                .to_str()
                .unwrap()
        );
    }

    #[test]
    fn test_sign_case4() {
        init();

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let body = r#"
            {"key": "value"}
        "#;

        let signature = sign_v1(
            "test-access-key-id",
            "test-access-key",
            Some("test-security-token"),
            Method::POST,
            "/logstores/test/shards/2",
            &mut headers,
            [
                ("type", "log"),
                ("count", "1000"),
                ("cursor", "MTczNzY2OTAzNjAxNzIxODQ1NA=="),
            ]
            .into(),
            Some(body.as_bytes()),
        )
        .unwrap();
        assert_eq!(
            signature,
            "LOG test-access-key-id:K3rw5WXRe77aZ/meSyEa8NNUYFc="
        );
        assert!(headers.contains_key(LOG_AUTHORIZATION));
        assert!(headers.contains_key(DATE));
        assert!(headers.contains_key(LOG_API_VERSION));
        assert!(headers.contains_key(LOG_SIGNATURE_METHOD));
        assert!(headers.contains_key(CONTENT_LENGTH));
        assert!(headers.contains_key(LOG_CONTENT_MD5));
        assert!(headers.contains_key(LOG_ACS_SECURITY_TOKEN));
        assert_eq!(
            "CE688F8D1AC3ED309BA9BE0A5ABAFCE5",
            headers.get(LOG_CONTENT_MD5).unwrap().to_str().unwrap()
        );
        assert_eq!(
            "test-security-token",
            headers
                .get(LOG_ACS_SECURITY_TOKEN)
                .unwrap()
                .to_str()
                .unwrap()
        );
    }
}
