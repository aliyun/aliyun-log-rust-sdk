use aliyun_log_producer::{
    Credentials, CredentialsError, CredentialsProvider, DeliveryError, DeliveryResult, Log,
    ProducerConfig, ProducerError, ThreadedProducer,
};
use std::time::Duration;

struct OfflineCredentials;
#[async_trait::async_trait]
impl CredentialsProvider for OfflineCredentials {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        Err(CredentialsError::provider(std::io::Error::other(
            "offline test",
        )))
    }
}

fn config() -> ProducerConfig {
    ProducerConfig::default()
        .with_endpoint("https://cn-hangzhou.log.aliyuncs.com")
        .with_credentials_provider(OfflineCredentials)
        .with_max_attempts(1)
}

#[test]
fn create_rejects_invalid_connection_and_settings() {
    for config in [
        ProducerConfig::default(),
        config().with_endpoint(""),
        config().with_endpoint("https://bad/path"),
        config().with_access_key("", "secret"),
        config().with_processing_workers(0),
        ProducerConfig::default().with_endpoint("cn-hangzhou.log.aliyuncs.com"),
    ] {
        assert!(matches!(
            ThreadedProducer::create(config),
            Err(ProducerError::Config(_))
        ));
    }
}

#[tokio::test]
async fn cloned_configs_create_independent_producers_without_registration() {
    let config = config();
    let first = ThreadedProducer::create(config.clone()).unwrap();
    let second = ThreadedProducer::create(config).unwrap();
    first.close().await.unwrap();
    let mut log = Log::from_unixtime(1_700_000_000);
    log.add_content_kv("message", "second producer remains open");
    second
        .writer("new-project", "new-store")
        .unwrap()
        .send(log)
        .unwrap();
    second.close().await.unwrap();
}

#[test]
fn config_debug_redacts_credentials() {
    let config = config().with_access_key("private-key-id", "private-key-secret");
    let debug = format!("{config:?}");
    assert!(!debug.contains("private-key-id"));
    assert!(!debug.contains("private-key-secret"));
}

// ThreadedProducer operations propagate one error type; callbacks report delivery separately.
async fn create_write_and_close() -> Result<DeliveryResult, ProducerError> {
    let producer = ThreadedProducer::create(config())?;
    let writer = producer.writer("project", "store")?;
    let mut log = Log::from_unixtime(1_700_000_000);
    log.add_content_kv("message", "test");
    let (tx, rx) = tokio::sync::oneshot::channel();
    writer.send_with_callback(log, move |result| {
        tx.send(result.clone()).unwrap();
    })?;
    producer.flush().await?;
    producer.close().await?;
    Ok(rx.await.unwrap())
}

#[tokio::test]
async fn producer_operations_and_callback_use_separate_error_types() {
    assert!(matches!(
        create_write_and_close().await.unwrap(),
        Err(DeliveryError::Credentials(_))
    ));
}

#[tokio::test]
async fn rejection_returns_original_log_without_exposing_payload_in_debug() {
    let producer = ThreadedProducer::create(config()).unwrap();
    let mut log = Log::from_unixtime(1_700_000_000);
    log.add_content_kv("message", "private-log-payload");
    let pointer = log.contents()[0].value().as_ptr();
    producer.close().await.unwrap();
    let error = producer
        .writer("project", "store")
        .unwrap()
        .send(log)
        .unwrap_err();
    assert!(matches!(error, ProducerError::Closed { .. }));
    assert!(!format!("{error:?}").contains("private-log-payload"));
    assert_eq!(error.log().unwrap().contents()[0].value().as_ptr(), pointer);
    let recovered = error.into_log().unwrap();
    assert_eq!(recovered.contents()[0].value().as_ptr(), pointer);
    assert_eq!(recovered.contents()[0].value(), "private-log-payload");
    producer.close().await.unwrap();
}

#[test]
fn non_admission_errors_have_no_log() {
    for error in [
        ProducerError::Config("invalid config".into()),
        ProducerError::Creation("initialization failed".into()),
        ProducerError::InvalidInput {
            reason: "invalid destination".into(),
        },
        ProducerError::PollBusy,
        ProducerError::Reentrant,
        ProducerError::Internal("worker failed".into()),
    ] {
        assert!(error.log().is_none());
        assert!(error.into_log().is_none());
    }
}

#[test]
fn all_send_methods_accept_hashmaps_and_return_converted_logs_on_rejection() {
    use aliyun_log_producer::SendOptions;
    use std::collections::HashMap;

    let producer = ThreadedProducer::create(config()).unwrap();
    let writer = producer.writer("project", "store").unwrap();
    producer.close_blocking().unwrap();
    let fields = || HashMap::from([("message".to_owned(), "from map".to_owned())]);
    let unexpected = |_: &DeliveryResult| panic!("rejected sends must not invoke callbacks");
    for result in [
        writer.send(fields()),
        writer.send_with_options(fields(), SendOptions::default()),
        writer.send_with_callback(fields(), unexpected),
        writer.send_with_options_and_callback(fields(), SendOptions::default(), unexpected),
    ] {
        let error = result.unwrap_err();
        assert!(matches!(error, ProducerError::Closed { .. }));
        let log = error.into_log().unwrap();
        assert_eq!(log.contents()[0].key(), "message");
        assert_eq!(log.contents()[0].value(), "from map");
        assert_eq!(*log.time_ns(), None);
    }
}

#[test]
fn delivery_error_display_contains_actionable_context() {
    let error = DeliveryError::Server {
        http_status: 403,
        error_code: "Unauthorized".into(),
        message: "permission denied".into(),
        request_id: Some("request-123".into()),
    };
    let display = error.to_string();
    for expected in ["403", "Unauthorized", "permission denied", "request-123"] {
        assert!(display.contains(expected));
    }
    let without_id = DeliveryError::Server {
        http_status: 500,
        error_code: "InternalServerError".into(),
        message: "try again".into(),
        request_id: None,
    };
    assert!(!without_id.to_string().contains("request_id"));
    for (error, expected) in [
        (
            DeliveryError::Network("offline".into()),
            "network error: offline",
        ),
        (
            DeliveryError::Credentials("unavailable".into()),
            "credentials error: unavailable",
        ),
        (DeliveryError::Timeout, "delivery timed out"),
        (
            DeliveryError::InvalidResponse("bad body".into()),
            "invalid service response: bad body",
        ),
        (
            DeliveryError::Internal("failed".into()),
            "internal delivery error: failed",
        ),
    ] {
        assert_eq!(error.to_string(), expected);
        assert_eq!(error.request_id(), None);
    }
}

#[test]
fn zero_is_not_allowed_for_delivery_and_retry_delays() {
    for invalid in [
        config().with_delivery_timeout(Duration::ZERO),
        config().with_base_backoff(Duration::ZERO),
        config().with_max_backoff(Duration::ZERO),
    ] {
        assert!(matches!(
            ThreadedProducer::create(invalid),
            Err(ProducerError::Config(_))
        ));
    }
}
