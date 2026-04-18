use std::sync::Arc;
use std::time::{Duration, SystemTime};

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tower::service_fn;

use aliyun_log_rust_sdk_producer::{
    DeliveryError, DeliveryReport, ExportBatch, LogRecord, ProducerBuilder, TowerSink,
};

const NUM_RECORDS: usize = 1_000_000;
const SEND_MANY_CHUNK_SIZE: usize = 100;

fn make_record(i: usize) -> LogRecord {
    LogRecord::new(SystemTime::UNIX_EPOCH)
        .field("message", format!("hello-{i}"))
        .field("level", "INFO")
}

fn build_records(n: usize) -> Vec<LogRecord> {
    (0..n).map(make_record).collect()
}

fn total_raw_size_bytes(records: &[LogRecord]) -> u64 {
    records.iter().map(|r| r.estimated_bytes() as u64).sum()
}

fn make_producer() -> impl std::future::Future<Output = aliyun_log_rust_sdk_producer::Producer> {
    let sink = TowerSink::new(service_fn(|batch: ExportBatch| async move {
        Ok::<_, DeliveryError>(DeliveryReport {
            batch_id: batch.batch_id,
            record_count: batch.records.len(),
            encoded_bytes: batch.estimated_bytes,
            retry_count: batch.retry_count,
            elapsed: batch.elapsed,
            request_id: None,
        })
    }));

    async move {
        ProducerBuilder::default()
            .batch_max_events(1024)
            .batch_max_bytes(1024 * 1024)
            .linger(Duration::from_millis(5))
            .channel_capacity(8192)
            .memory_limit_bytes(256 * 1024 * 1024)
            .concurrency(4)
            .sink(Arc::new(sink))
            .build()
            .await
            .unwrap()
    }
}

async fn run_send(records: &[LogRecord]) {
    let producer = make_producer().await;
    for record in records.iter().cloned() {
        producer.send(record).await.unwrap();
    }
    producer.flush().await.unwrap();
}

async fn run_send_many(records: &[LogRecord]) {
    let producer = make_producer().await;
    for chunk in records.chunks(SEND_MANY_CHUNK_SIZE) {
        producer.send_many(chunk.iter().cloned()).await.unwrap();
    }
    producer.flush().await.unwrap();
}

fn bench_case<F, Fut>(
    c: &mut Criterion,
    group_name: &str,
    case_name: &str,
    records: Arc<Vec<LogRecord>>,
    raw_bytes: u64,
    run: F,
) where
    F: Fn(Arc<Vec<LogRecord>>) -> Fut + Copy + 'static,
    Fut: std::future::Future<Output = ()>,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .unwrap();

    let mut logs_group = c.benchmark_group(format!("{group_name}_logs"));
    logs_group.sample_size(10);
    logs_group.measurement_time(Duration::from_secs(3));
    logs_group.warm_up_time(Duration::from_secs(1));
    logs_group.throughput(Throughput::Elements(records.len() as u64));
    logs_group.bench_with_input(
        BenchmarkId::new(case_name, records.len()),
        &records,
        |b, data| {
            let data = Arc::clone(data);
            b.to_async(&rt).iter(|| run(Arc::clone(&data)))
        },
    );
    logs_group.finish();

    let mut bytes_group = c.benchmark_group(format!("{group_name}_raw_bytes"));
    bytes_group.sample_size(10);
    bytes_group.measurement_time(Duration::from_secs(3));
    bytes_group.warm_up_time(Duration::from_secs(1));
    bytes_group.throughput(Throughput::Bytes(raw_bytes));
    bytes_group.bench_with_input(
        BenchmarkId::new(case_name, raw_bytes),
        &records,
        |b, data| {
            let data = Arc::clone(data);
            b.to_async(&rt).iter(|| run(Arc::clone(&data)))
        },
    );
    bytes_group.finish();
}

fn bench_mock_sink(c: &mut Criterion) {
    let records = Arc::new(build_records(NUM_RECORDS));
    let raw_bytes = total_raw_size_bytes(&records);

    bench_case(
        c,
        "mock_sink_send",
        "producer_flush",
        Arc::clone(&records),
        raw_bytes,
        |records| async move { run_send(records.as_slice()).await },
    );

    bench_case(
        c,
        "mock_sink_send_many_100",
        "producer_flush",
        Arc::clone(&records),
        raw_bytes,
        |records| async move { run_send_many(records.as_slice()).await },
    );
}

criterion_group!(benches, bench_mock_sink);
criterion_main!(benches);
