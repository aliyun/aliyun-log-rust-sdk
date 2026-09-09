use super::*;
use crate::{async_trait, Credentials, CredentialsError, CredentialsProvider};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::SystemTime,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

struct RotatingProvider(AtomicUsize);

#[async_trait]
impl CredentialsProvider for RotatingProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        let attempt = self.0.fetch_add(1, Ordering::SeqCst);
        let credentials = Credentials::new(format!("id-{attempt}"), format!("secret-{attempt}"))?;
        Ok(match attempt {
            0 | 1 => credentials
                .with_security_token(format!("token-{attempt}"))
                .with_expiration(SystemTime::now() + Duration::from_secs(1)),
            _ => credentials,
        })
    }
}

async fn read_request(stream: &mut TcpStream) -> (String, HeaderMap, Vec<u8>) {
    let mut data = Vec::new();
    let end = loop {
        let mut chunk = [0; 4096];
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(count > 0);
        data.extend_from_slice(&chunk[..count]);
        if let Some(end) = data.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = String::from_utf8(data[..end].to_vec()).unwrap();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap().to_string();
    let mut headers = HeaderMap::new();
    for line in lines.filter(|line| !line.is_empty()) {
        let (key, value) = line.split_once(':').unwrap();
        headers.insert(
            http::HeaderName::from_bytes(key.as_bytes()).unwrap(),
            value.trim().parse().unwrap(),
        );
    }
    let length: usize = headers
        .get(http::header::CONTENT_LENGTH)
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    while data.len() < end + length {
        let mut chunk = [0; 4096];
        let count = stream.read(&mut chunk).await.unwrap();
        assert!(count > 0);
        data.extend_from_slice(&chunk[..count]);
    }
    (request_line, headers, data[end..end + length].to_vec())
}

#[tokio::test]
async fn http_retries_resign_with_rotated_keys_and_replace_or_remove_sts_token() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for attempt in 0..3 {
            let (mut stream, _) = listener.accept().await.unwrap();
            requests.push(read_request(&mut stream).await);
            let (status, body) = if attempt < 2 {
                (
                    "503 Service Unavailable",
                    r#"{"errorCode":"ServerBusy","errorMessage":"retry"}"#,
                )
            } else {
                ("200 OK", "done")
            };
            stream.write_all(format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()
            ).as_bytes()).await.unwrap();
        }
        requests
    });

    let provider = Arc::new(RotatingProvider(AtomicUsize::new(0)));
    let mut config = Config::builder()
        .endpoint(format!("http://{address}"))
        .credentials_provider(provider.clone())
        .build()
        .unwrap();
    config.base_retry_backoff = Duration::from_millis(1100);
    let handle = Handle {
        config,
        http_client: reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap(),
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    headers.insert(
        "x-acs-security-token",
        http::HeaderValue::from_static("unrelated-old-token"),
    );
    headers.insert(
        http::header::AUTHORIZATION,
        http::HeaderValue::from_static("old-authorization"),
    );
    let body = bytes::Bytes::from_static(br#"{"message":"hello"}"#);
    let response = tokio::time::timeout(
        Duration::from_secs(15),
        handle.send_http(
            http::Method::POST,
            format!("http://{address}"),
            "/logs",
            Some(vec![("type".into(), "log".into())]),
            Some(body.clone()),
            headers,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(response.decompressed, b"done");
    let requests = server.await.unwrap();
    assert_eq!(provider.0.load(Ordering::SeqCst), 3);
    for (attempt, (line, headers, sent_body)) in requests.iter().enumerate() {
        assert_eq!(line, "POST /logs?type=log HTTP/1.1");
        assert_eq!(sent_body.as_slice(), body.as_ref());
        assert!(headers[http::header::AUTHORIZATION]
            .to_str()
            .unwrap()
            .starts_with(&format!("LOG id-{attempt}:")));
        if attempt < 2 {
            assert_eq!(
                headers["x-acs-security-token"].to_str().unwrap(),
                format!("token-{attempt}")
            );
        } else {
            assert!(!headers.contains_key("x-acs-security-token"));
        }
        assert_eq!(headers["content-md5"], requests[0].1["content-md5"]);
    }
    assert_ne!(
        requests[0].1[http::header::DATE],
        requests[2].1[http::header::DATE]
    );
}

struct FailingProvider(AtomicUsize);
#[async_trait]
impl CredentialsProvider for FailingProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(CredentialsError::provider(anyhow::anyhow!("unavailable")))
    }
}

#[tokio::test]
async fn missing_credentials_return_typed_errors_without_sending_http() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let provider = Arc::new(FailingProvider(AtomicUsize::new(0)));
    let config = Config::builder()
        .endpoint(format!("http://{}", listener.local_addr().unwrap()))
        .credentials_provider(provider.clone())
        .build()
        .unwrap();
    let client = Client::from_config(config).unwrap();
    assert!(matches!(
        client.list_projects(0, 10).send().await,
        Err(Error::Credentials(CredentialsError::Provider(_)))
    ));
    assert!(matches!(
        client.list_projects(0, 10).send().await,
        Err(Error::Credentials(CredentialsError::Throttled { .. }))
    ));
    assert_eq!(provider.0.load(Ordering::SeqCst), 3);
    assert!(
        tokio::time::timeout(Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
}
