use aliyun_log_producer::{Log, ProducerConfig, ThreadedProducer};
use aliyun_log_rust_sdk::{Credentials, CredentialsError, CredentialsProvider};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

struct CountingCredentials(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl CredentialsProvider for CountingCredentials {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Credentials::new("test-id", "test-secret")
    }
}

#[tokio::test]
async fn dynamic_writers_share_http_connection_and_credentials_cache() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        // All requests must use this connection; a per-writer client would open another.
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut pending = Vec::new();
        let mut requests = Vec::new();
        for _ in 0..3 {
            let header_end = loop {
                if let Some(index) = pending.windows(4).position(|w| w == b"\r\n\r\n") {
                    break index + 4;
                }
                assert_ne!(socket.read_buf(&mut pending).await.unwrap(), 0);
            };
            let headers = String::from_utf8(pending[..header_end].to_vec()).unwrap();
            let length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            while pending.len() < header_end + length {
                assert_ne!(socket.read_buf(&mut pending).await.unwrap(), 0);
            }
            let raw = zstd::stream::decode_all(&pending[header_end..header_end + length]).unwrap();
            assert!(!raw.is_empty());
            pending.drain(..header_end + length);
            requests.push(headers.lines().next().unwrap().to_owned());
            socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nx-log-requestid: test-request\r\n\r\n").await.unwrap();
        }
        requests
    });
    let fetches = Arc::new(AtomicUsize::new(0));
    let producer = ThreadedProducer::create(
        ProducerConfig::default()
            // Client prefixes project 127, yielding loopback 127.0.0.1.
            .with_endpoint(format!("http://0.0.1:{port}"))
            .with_credentials_provider(CountingCredentials(fetches.clone()))
            .with_max_attempts(1)
            .with_delivery_timeout(Duration::from_secs(5)),
    )
    .unwrap();
    assert_eq!(
        fetches.load(Ordering::Relaxed),
        0,
        "create must not fetch credentials"
    );
    for store in ["first", "second", "first"] {
        // A new destination appears after delivery has already started; dropped
        // writer handles can be obtained again without losing their cached target.
        let writer = producer.writer("127", store).unwrap();
        let mut log = Log::from_unixtime(1_700_000_000);
        log.add_content_kv("message", store);
        let (tx, rx) = tokio::sync::oneshot::channel();
        writer
            .send_with_callback(log, move |r| {
                tx.send(r).unwrap();
            })
            .unwrap();
        producer.flush().await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }
    producer.close().await.unwrap();
    let requests = tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        requests,
        [
            "POST /logstores/first/shards/lb HTTP/1.1",
            "POST /logstores/second/shards/lb HTTP/1.1",
            "POST /logstores/first/shards/lb HTTP/1.1",
        ]
    );
    assert_eq!(fetches.load(Ordering::Relaxed), 1);
}
