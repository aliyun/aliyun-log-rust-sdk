use std::time::Duration;

use crate::config::Config;
use crate::utils::{user_agent, ValueGetter};
use crate::{
    common::*, CompressionError, ConfigError, RequestError, RequestErrorKind, ResponseErrorKind,
    ResponseResult,
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

pub(crate) use crate::macros::*;

mod pull_logs;
pub use pull_logs::*;
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

        // do request signing
        let query_params = query_params.unwrap_or_default();

        sign_v1(
            &self.config.access_key_id,
            &self.config.access_key_secret,
            self.config.security_token.as_deref(),
            method.clone(),
            path.as_ref(),
            &mut headers,
            query_params.into(),
            body.as_deref(),
        )
        .map_err(RequestErrorKind::from)
        .map_err(RequestError::from)?;

        let max_retry = self.config.max_retry + 1;
        for i in 0..max_retry {
            // here body.clone() is O(1), no underlying data is copied
            match self
                .send_signed_http(&method, &url, &headers, body.clone())
                .await
            {
                Ok(resp) => {
                    return Ok(resp);
                }
                Err(err) => {
                    debug!("fail to send on {} err: {:?}", i, &err.to_string());
                    if !self.should_retry(&err) || i + 1 >= max_retry {
                        return Err(err);
                    }
                }
            }

            let backoff = exponential_backoff(
                self.config.base_retry_backoff,
                i,
                self.config.max_retry_backoff,
            );
            sleep(backoff).await;
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
                let resp_body = response.text().await?;
                Err(Error::server_error(
                    status,
                    request_id,
                    resp_body.as_bytes(),
                ))
            }
        }
    }

    fn should_retry(&self, err: &Error) -> bool {
        match err {
            Error::Network(_) => true,
            Error::Server { http_status, .. } => *http_status >= 500 && *http_status <= 503,
            _ => false,
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
        let raw_size = headers.get_i32_or_default(&LOG_BODY_RAW_SIZE, 0);
        if raw_size == 0 {
            return Ok(Vec::new());
        }

        decompress(body.into(), &compress_type, raw_size as usize).map_err(|source| {
            let request_id = headers.get_str(LOG_REQUEST_ID);
            ResponseErrorKind::Decompression {
                source,
                compress_type,
                request_id,
            }
            .into()
        })
    }
}

fn exponential_backoff(base_delay: Duration, retry_count: u32, max_delay: Duration) -> Duration {
    let exp_delay = base_delay * 2u32.pow(retry_count);
    std::cmp::min(exp_delay, max_delay)
}

const DEFAULT_POOL_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(55);

pub type BoxFuture<T> =
    ::std::pin::Pin<::std::boxed::Box<dyn ::std::future::Future<Output = T> + ::std::marker::Send>>;

pub type ResponseResultBoxFuture<B> = BoxFuture<Result<Response<B>, Error>>;
