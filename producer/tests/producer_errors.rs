use aliyun_log_producer::{
    Credentials, CredentialsError, CredentialsProvider, DeliveryError, DeliveryResult, Log,
    Producer, ProducerConfig, ProducerError,
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
            Producer::create(config),
            Err(ProducerError::Config(_))
        ));
    }
}

#[tokio::test]
async fn cloned_configs_create_independent_producers_without_registration() {
    let config = config();
    let first = Producer::create(config.clone()).unwrap();
    let second = Producer::create(config).unwrap();
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

// Producer operations propagate one error type; callbacks report delivery separately.
async fn create_write_and_close() -> Result<DeliveryResult, ProducerError> {
    let producer = Producer::create(config())?;
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
    let producer = Producer::create(config()).unwrap();
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
fn all_send_methods_return_logs_on_rejection() {
    use aliyun_log_producer::SendOptions;

    let producer = Producer::create(config()).unwrap();
    let writer = producer.writer("project", "store").unwrap();
    producer.close_blocking().unwrap();
    let entry = || aliyun_log_producer::log!("message": "rejected log");
    let unexpected = |_: &DeliveryResult| panic!("rejected sends must not invoke callbacks");
    for result in [
        writer.send(entry()),
        writer.send_with_options(entry(), SendOptions::default()),
        writer.send_with_callback(entry(), unexpected),
        writer.send_with_options_and_callback(entry(), SendOptions::default(), unexpected),
    ] {
        let error = result.unwrap_err();
        assert!(matches!(error, ProducerError::Closed { .. }));
        let log = error.into_log().unwrap();
        assert_eq!(log.contents()[0].key(), "message");
        assert_eq!(log.contents()[0].value(), "rejected log");
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
fn duration_config_bounds_are_inclusive() {
    let setters = [
        (
            "linger",
            ProducerConfig::with_linger as fn(ProducerConfig, Duration) -> ProducerConfig,
            Duration::from_millis(10),
            Duration::from_secs(365 * 24 * 3600),
        ),
        (
            "base_backoff",
            ProducerConfig::with_base_backoff,
            Duration::from_millis(100),
            Duration::from_secs(60),
        ),
        (
            "max_backoff",
            ProducerConfig::with_max_backoff,
            Duration::from_millis(100),
            Duration::from_secs(600),
        ),
        (
            "delivery_timeout",
            ProducerConfig::with_delivery_timeout,
            Duration::from_secs(60),
            Duration::from_secs(7 * 24 * 3600),
        ),
    ];
    for (name, setter, min, max) in setters {
        let config = config()
            .with_base_backoff(Duration::from_millis(100))
            .with_max_backoff(Duration::from_secs(600));
        for value in [
            Duration::ZERO,
            min - Duration::from_nanos(1),
            max + Duration::from_nanos(1),
        ] {
            assert!(
                matches!(Producer::create(setter(config.clone(), value)),
                    Err(ProducerError::Config(message)) if message.contains(name)),
                "{name} accepted {value:?}"
            );
        }
        for value in [min, max] {
            let producer = Producer::create(setter(config.clone(), value)).unwrap();
            producer.close_blocking().unwrap();
        }
    }
}

#[test]
fn count_config_bounds_and_backoff_order_are_validated() {
    for (name, invalid) in [
        ("max_attempts", config().with_max_attempts(0)),
        ("processing_workers", config().with_processing_workers(0)),
        ("callback_capacity", config().with_callback_capacity(0)),
        ("callback_capacity", config().with_callback_capacity(1023)),
        (
            "callback_capacity",
            config().with_callback_capacity(1024 * 1024 + 1),
        ),
        (
            "base_backoff",
            config()
                .with_base_backoff(Duration::from_millis(101))
                .with_max_backoff(Duration::from_millis(100)),
        ),
    ] {
        assert!(matches!(Producer::create(invalid),
            Err(ProducerError::Config(message)) if message.contains(name)));
    }
    for capacity in [1024, 1024 * 1024] {
        let producer = Producer::create(
            config()
                .with_max_attempts(1)
                .with_processing_workers(1)
                .with_callback_capacity(capacity)
                .with_base_backoff(Duration::from_millis(100))
                .with_max_backoff(Duration::from_millis(100)),
        )
        .unwrap();
        producer.close_blocking().unwrap();
    }
}
