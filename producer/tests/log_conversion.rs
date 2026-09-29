use aliyun_log_producer::Log;

#[test]
fn application_defined_from_conversion_works_with_send() {
    use aliyun_log_producer::{Producer, ProducerConfig};

    struct Event(String);
    impl From<Event> for Log {
        fn from(event: Event) -> Self {
            aliyun_log_producer::log!("message": event.0)
        }
    }

    let producer = Producer::create(
        ProducerConfig::default()
            .with_endpoint("example.com")
            .with_access_key("test-id", "test-secret"),
    )
    .unwrap();
    let writer = producer.writer("project", "store").unwrap();
    producer.close_blocking().unwrap();
    let rejected = writer
        .send(Event("custom event".into()))
        .unwrap_err()
        .into_log()
        .unwrap();
    assert_eq!(rejected.contents()[0].value(), "custom event");
}
