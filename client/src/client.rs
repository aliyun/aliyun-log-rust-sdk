use std::time::Duration;

use crate::config::Config;
use crate::utils::{user_agent, ValueGetter};
use crate::{
    common::*, CompressionError, ConfigError, RequestError, RequestErrorKind, ResponseError,
    ResponseErrorKind, ResponseResult,
};
use aliyun_log_sdk_sign::sign_v1;
use http::header::USER_AGENT;
use http::HeaderMap;

use log::debug;

use tokio::time::sleep;

use crate::{
    compress::{compress, decompress, CompressType},
    error::{Error, Result},
};
mod consumer_group;
pub use consumer_group::*;

mod project;
pub use project::*;

mod logstore;
pub use logstore::*;

mod index;
pub use index::*;

pub(crate) use crate::macros::*;

mod pull_logs;
pub use pull_logs::*;
mod pull_logs_raw;
pub use pull_logs_raw::*;
mod put_logs;
pub use put_logs::*;
mod get_cursor;
pub use get_cursor::*;
mod list_shards;
pub use list_shards::*;
mod get_logs;
use crate::request::Request;
use crate::response::{DecompressedResponse, FromHttpResponse, Response};
pub use get_logs::*;
mod put_logs_raw;
pub use put_logs_raw::*;

/// Aliyun Log Service client
///
/// # Examples
///
/// A simple example of creating a new client:
/// ```
/// # async fn wrapper() -> aliyun_log_rust_sdk::Result<()> {
/// use aliyun_log_rust_sdk::{Client, Config, FromConfig};
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .access_key("access_key_id", "access_key_secret")
///     .build()?;
/// let client = Client::from_config(config)?;
/// # Ok(())
/// # }
/// ```
///
/// Example of creating a new client with security token and request_timeout:
/// ```
/// # async fn wrapper() -> aliyun_log_rust_sdk::Result<()> {
/// use aliyun_log_rust_sdk::{Client, Config, FromConfig};
/// let config = Config::builder()
///     .endpoint("cn-hangzhou.log.aliyuncs.com")
///     .sts("access_key_id", "access_key_secret", "security_token")
///     .request_timeout(std::time::Duration::from_secs(60))
///     .build()?;
/// let client = Client::from_config(config)?;
/// # Ok(())
/// # }
/// ```
///
/// For more configuration options, see [`ConfigBuilder`](crate::config::ConfigBuilder).
#[derive(Clone)]
pub struct Client {
    handle: HandleRef,
}

pub(crate) struct Handle {
    config: Config,
    http_client: reqwest::Client,
}

pub(crate) type HandleRef = std::sync::Arc<Handle>;

pub(crate) type BuildResult<T> = std::result::Result<(HandleRef, T), RequestError>;

pub trait FromConfig: Sized {
    fn from_config(config: Config) -> Result<Self, ConfigError>;
}

impl FromConfig for Client {
    fn from_config(config: Config) -> Result<Self, ConfigError> {
        let http_client = reqwest::Client::builder()
            .connect_timeout(config.connection_timeout)
            .timeout(config.request_timeout)
            .pool_idle_timeout(DEFAULT_POOL_IDLE_TIMEOUT)
            .build()?;
        let handle = HandleRef::new(Handle {
            config,
            http_client,
        });
        Ok(Self { handle })
    }
}

impl Handle {
    pub(crate) async fn send<R>(&self, request: R) -> Result<Response<R::ResponseBody>>
    where
        R: Request,
    {
        let path = request.path();
        let host = self.build_host(request.project());
        let query_params = request.query_params();
        let method = R::HTTP_METHOD;
        let mut headers = request.headers();
        if let Some(content_type) = R::CONTENT_TYPE {
            headers.insert(http::header::CONTENT_TYPE, content_type);
        }

        let body = self.get_request_body(&request, &mut headers)?;
        if !headers.contains_key(LOG_BODY_RAW_SIZE) {
            let body_len = match body {
                None => 0,
                Some(ref b) => b.len(),
            };

            headers.insert(
                LOG_BODY_RAW_SIZE,
                body_len
                    .to_string()
                    .parse()
                    .expect("fail to inser bodyRawSize into header"),
            );
        }

        let resp = self
            .send_http(method, host, path, query_params, body, headers)
            .await?;

        let resp_bytes: bytes::Bytes = resp.decompressed.into();
        let resp_body = <R::ResponseBody as FromHttpResponse>::try_from(resp_bytes, &resp.headers)?;
        Ok(Response {
            body: resp_body,
            headers: resp.headers,
            status: resp.status,
        })
    }

    fn get_request_body<R>(
        &self,
        request: &R,
        headers: &mut http::HeaderMap,
    ) -> Result<Option<bytes::Bytes>>
    where
        R: Request,
    {
        let body = request.body()?;
        if body.is_none() {
            return Ok(None);
        }
        if R::COMPRESS_TYPE.is_none() {
            return Ok(body);
        }
        let compressed = self
            .do_compress(&R::COMPRESS_TYPE.unwrap(), body.unwrap(), headers)
            .map_err(RequestErrorKind::from)
            .map_err(RequestError::from)?;

        Ok(Some(compressed.into()))
    }

    async fn send_http(
        &self,
        method: http::Method,
        host: impl AsRef<str>,
        path: impl AsRef<str>,
        query_params: Option<Vec<(String, String)>>,
        body: Option<bytes::Bytes>,
        mut headers: http::HeaderMap,
    ) -> Result<DecompressedResponse> {
        if !headers.contains_key(USER_AGENT) {
            headers.insert(
                USER_AGENT,
                user_agent()
                    .parse()
                    .expect("fail to insert UserAgent into headers"),
            );
        }

        // prepare http request parameters
        let url = self.build_url(host.as_ref(), path.as_ref(), &query_params)?;

        let query_params = query_params.unwrap_or_default();
        let policy = &self.config.retry_policy;
        let started = std::time::Instant::now();
        for attempt in 0..=policy.max_retries {
            let credentials = self
                .config
                .credentials_provider
                .credentials()
                .await
                .map_err(Error::CredentialsProvider)?;
            if !credentials.validate() {
                return Err(ConfigError::InvalidAccessKey.into());
            }
            let mut attempt_headers = headers.clone();
            sign_v1(
                credentials.access_key_id(),
                credentials.access_key_secret(),
                credentials.security_token(),
                method.clone(),
                path.as_ref(),
                &mut attempt_headers,
                query_params.clone().into(),
                body.as_deref(),
            )
            .map_err(RequestErrorKind::from)
            .map_err(RequestError::from)?;

            // here body.clone() is O(1), no underlying data is copied
            match self
                .send_signed_http(&method, &url, &attempt_headers, body.clone())
                .await
            {
                Ok(resp) => {
                    return Ok(resp);
                }
                Err(err) => {
                    debug!("request attempt {attempt} failed: {}", err);
                    if !self.should_retry(&method, &err) || attempt >= policy.max_retries {
                        return Err(err);
                    }
                    let backoff = if let Some(retry_after) = err.retry_after() {
                        retry_after
                    } else {
                        let backoff =
                            exponential_backoff(policy.base_backoff, attempt, policy.max_backoff);
                        if policy.jitter {
                            full_jitter(backoff)
                        } else {
                            backoff
                        }
                    };
                    if started.elapsed().saturating_add(backoff) >= policy.max_elapsed {
                        return Err(err);
                    }
                    sleep(backoff).await;
                }
            }
        }
        Err(Error::Other(anyhow::anyhow!(
            "unreachable, this is a bug, please open an issue to report it."
        )))
    }

    async fn send_signed_http(
        &self,
        method: &http::Method,
        url: &url::Url,
        headers: &HeaderMap,
        body: Option<bytes::Bytes>,
    ) -> Result<DecompressedResponse> {
        let req = match *method {
            http::Method::POST => self.http_client.post(url.clone()),
            http::Method::GET => self.http_client.get(url.clone()),
            http::Method::PUT => self.http_client.put(url.clone()),
            http::Method::DELETE => self.http_client.delete(url.clone()),
            _ => {
                return Err(Error::Other(anyhow::anyhow!("Unsupported HTTP method: {method:?}, this is a bug, please open an issue to report it.")));
            }
        };

        let req = match body {
            Some(b) => req.body(b).headers(headers.clone()),
            None => req.headers(headers.clone()),
        };
        self.send_reqwest(req.build()?).await
    }

    async fn send_reqwest(&self, request: reqwest::Request) -> Result<DecompressedResponse> {
        let response = self.http_client.execute(request).await?;
        let status = response.status();
        match status {
            http::status::StatusCode::OK => {
                let resp_headers = response.headers().to_owned();
                let resp_body = response.bytes().await?;
                let decompressed = self.do_decompress(resp_body, &resp_headers)?;
                Ok(DecompressedResponse {
                    headers: resp_headers,
                    status,
                    decompressed,
                })
            }
            _ => {
                let request_id = response.headers().get_str(LOG_REQUEST_ID);
                let retry_after = parse_retry_after(response.headers());
                let resp_body = response.bytes().await?;
                Err(Error::server_error(
                    status,
                    request_id,
                    resp_body,
                    retry_after,
                ))
            }
        }
    }

    fn should_retry(&self, method: &http::Method, err: &Error) -> bool {
        let is_read = *method == http::Method::GET;
        if matches!(err, Error::Network(_)) {
            // A failed write may have reached SLS even when its response was lost.
            return is_read;
        }
        let status = match err {
            Error::Server { http_status, .. } | Error::HttpResponse { http_status, .. } => {
                *http_status
            }
            _ => return false,
        };
        if is_read {
            status == 429 || (500..=599).contains(&status)
        } else {
            status == 429 || matches!(status, 500 | 502 | 503)
        }
    }

    fn build_host(&self, project: Option<&str>) -> String {
        match project {
            Some(project) => format!(
                "{}{}.{}",
                self.config.endpoint.scheme, project, self.config.endpoint.domain
            ),
            None => format!(
                "{}{}",
                self.config.endpoint.scheme, self.config.endpoint.domain
            ),
        }
    }

    fn build_url(
        &self,
        host: &str,
        path: &str,
        query_params: &Option<Vec<(String, String)>>,
    ) -> Result<url::Url, ConfigError> {
        let result = match query_params {
            Some(query_params) if query_params.is_empty() => {
                url::Url::parse(&format!("{host}{path}"))
            }
            None => url::Url::parse(&format!("{host}{path}")),
            Some(query_params) => {
                url::Url::parse_with_params(&format!("{host}{path}"), query_params)
            }
        };
        Ok(result?)
    }

    fn do_compress(
        &self,
        compress_type: &CompressType,
        body: impl AsRef<[u8]>,
        headers: &mut http::HeaderMap,
    ) -> std::result::Result<Vec<u8>, CompressionError> {
        let body = body.as_ref();
        let body_raw_size = body.len();
        headers.insert(
            LOG_BODY_RAW_SIZE,
            body_raw_size
                .to_string()
                .parse()
                .expect("fail to insert bodyRawSize into header"),
        );
        headers.insert(
            LOG_COMPRESS_TYPE,
            compress_type
                .to_string()
                .parse()
                .expect("fail to insert compressType into header"),
        );

        compress(body, compress_type)
    }

    fn do_decompress(
        &self,
        body: impl Into<Vec<u8>>,
        headers: &http::HeaderMap,
    ) -> ResponseResult<Vec<u8>> {
        let compress_type = headers.get_str_or_default(&LOG_COMPRESS_TYPE, "");
        if compress_type.is_empty() {
            return Ok(body.into());
        }
        let body = body.into();
        let request_id = headers.get_str(LOG_REQUEST_ID);
        let raw_size_value = headers.get(&LOG_BODY_RAW_SIZE).ok_or_else(|| {
            ResponseError::from(ResponseErrorKind::InvalidCompressionHeader {
                header: "x-log-bodyrawsize",
                reason: "header is missing".into(),
                request_id: request_id.clone(),
            })
        })?;
        let raw_size = raw_size_value
            .to_str()
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| {
                ResponseError::from(ResponseErrorKind::InvalidCompressionHeader {
                    header: "x-log-bodyrawsize",
                    reason: "value is not a non-negative integer".into(),
                    request_id: request_id.clone(),
                })
            })?;
        if raw_size > MAX_DECOMPRESSED_BODY_SIZE || raw_size > i32::MAX as usize {
            return Err(ResponseErrorKind::DecompressedBodyTooLarge {
                raw_size,
                limit: MAX_DECOMPRESSED_BODY_SIZE.min(i32::MAX as usize),
                request_id,
            }
            .into());
        }
        if raw_size == 0 {
            // SLS may return a non-empty compression sentinel for an empty
            // result. Match the Go SDK and treat raw size zero as empty.
            return Ok(Vec::new());
        }

        let decompressed = decompress(body, &compress_type, raw_size).map_err(|source| {
            let request_id = headers.get_str(LOG_REQUEST_ID);
            ResponseError::from(ResponseErrorKind::Decompression {
                source,
                compress_type,
                request_id,
            })
        })?;
        if decompressed.len() != raw_size {
            return Err(ResponseErrorKind::DecompressedSizeMismatch {
                expected: raw_size,
                actual: decompressed.len(),
                request_id: headers.get_str(LOG_REQUEST_ID),
            }
            .into());
        }
        Ok(decompressed)
    }
}

fn exponential_backoff(base_delay: Duration, retry_count: u32, max_delay: Duration) -> Duration {
    let multiplier = 2u32.checked_pow(retry_count).unwrap_or(u32::MAX);
    let exp_delay = base_delay.checked_mul(multiplier).unwrap_or(max_delay);
    std::cmp::min(exp_delay, max_delay)
}

fn full_jitter(max_delay: Duration) -> Duration {
    if max_delay.is_zero() {
        return max_delay;
    }
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u128;
    let upper = max_delay.as_nanos().min(u64::MAX as u128);
    let nanos = seed % (upper + 1);
    Duration::from_nanos(nanos as u64)
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(http::header::RETRY_AFTER)?.to_str().ok()?;
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let deadline = httpdate::parse_http_date(value).ok()?;
    deadline.duration_since(std::time::SystemTime::now()).ok()
}

const DEFAULT_POOL_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(55);
const MAX_DECOMPRESSED_BODY_SIZE: usize = 512 * 1024 * 1024;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Config, Credentials, CredentialsFuture, CredentialsProvider, FromConfig, RetryPolicy,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn client() -> Client {
        Client::from_config(
            Config::builder()
                .endpoint("localhost")
                .access_key("id", "secret")
                .build()
                .expect("valid test config"),
        )
        .expect("valid test client")
    }

    #[test]
    fn compressed_response_requires_valid_raw_size() {
        let client = client();
        let mut headers = HeaderMap::new();
        headers.insert(LOG_COMPRESS_TYPE, http::HeaderValue::from_static("lz4"));

        assert!(client
            .handle
            .do_decompress(vec![1, 2, 3], &headers)
            .is_err());

        headers.insert(LOG_BODY_RAW_SIZE, http::HeaderValue::from_static("0"));
        assert_eq!(
            client
                .handle
                .do_decompress(vec![1], &headers)
                .expect("SLS uses a non-empty sentinel for some empty responses"),
            Vec::<u8>::new()
        );
    }

    #[test]
    fn retry_policy_distinguishes_reads_and_writes() {
        let client = client();
        let error = Error::HttpResponse {
            http_status: 504,
            request_id: None,
            body: bytes::Bytes::new(),
            retry_after: None,
        };
        assert!(client.handle.should_retry(&http::Method::GET, &error));
        assert!(!client.handle.should_retry(&http::Method::POST, &error));

        let error = Error::HttpResponse {
            http_status: 502,
            request_id: None,
            body: bytes::Bytes::new(),
            retry_after: None,
        };
        assert!(client.handle.should_retry(&http::Method::POST, &error));
    }

    struct CountingCredentials {
        calls: Arc<AtomicUsize>,
    }

    impl CredentialsProvider for CountingCredentials {
        fn credentials(&self) -> CredentialsFuture<'_> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(Credentials::new("id", "secret", None)) })
        }
    }

    #[tokio::test]
    async fn malformed_503_is_retried_and_credentials_are_refreshed() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let server = tokio::spawn(async move {
            for attempt in 0..2 {
                let (mut stream, _) = listener.accept().await.expect("accept request");
                let mut request = vec![0; 4096];
                let _ = stream.read(&mut request).await.expect("read request");
                let response = if attempt == 0 {
                    "HTTP/1.1 503 Service Unavailable\r\nContent-Length: 11\r\nConnection: close\r\n\r\nbad gateway"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                };
                stream
                    .write_all(response.as_bytes())
                    .await
                    .expect("write response");
            }
        });

        let calls = Arc::new(AtomicUsize::new(0));
        let config = Config::builder()
            .endpoint(format!("http://{address}"))
            .credentials_provider(CountingCredentials {
                calls: Arc::clone(&calls),
            })
            .retry_policy(
                RetryPolicy::new()
                    .max_retries(1)
                    .base_backoff(Duration::ZERO)
                    .max_backoff(Duration::ZERO)
                    .max_elapsed(Duration::from_secs(1))
                    .jitter(false),
            )
            .build()
            .expect("valid config");
        let client = Client::from_config(config).expect("valid client");

        client
            .handle
            .send_http(
                http::Method::GET,
                format!("http://{address}"),
                "/",
                None,
                None,
                HeaderMap::new(),
            )
            .await
            .expect("second attempt succeeds");
        server.await.expect("test server task");
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
}
