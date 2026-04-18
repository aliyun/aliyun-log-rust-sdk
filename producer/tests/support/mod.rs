use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aliyun_log_rust_sdk::{get_cursor_models::CursorPos, Client, Config, FromConfig};
use aliyun_log_rust_sdk_producer::{LogRecord, Producer, ProducerBuilder};
use serde::Deserialize;
use tokio::time::{sleep, Instant};

#[derive(Debug, Clone, Deserialize)]
pub struct TestEnvironment {
    pub access_key_id: String,
    pub access_key_secret: String,
    pub endpoint: String,
    pub project: String,
    pub logstore: String,
}

static TEST_ENV: OnceLock<Option<TestEnvironment>> = OnceLock::new();

pub fn init_logger() {
    let mut builder = env_logger::Builder::from_default_env();
    builder.is_test(true);

    if std::env::var_os("PRODUCER_TEST_DEBUG").is_some() {
        builder.filter_level(log::LevelFilter::Debug);
        builder.filter_module("aliyun_log_rust_sdk", log::LevelFilter::Debug);
        builder.filter_module("aliyun_log_rust_sdk_producer", log::LevelFilter::Debug);
    } else if std::env::var_os("RUST_LOG").is_none() {
        builder.filter_level(log::LevelFilter::Info);
    }

    let _ = builder.try_init();
}

pub fn test_env() -> Option<&'static TestEnvironment> {
    TEST_ENV
        .get_or_init(|| envy::from_env::<TestEnvironment>().ok())
        .as_ref()
}

pub fn require_env() -> Option<&'static TestEnvironment> {
    let env = test_env();
    if env.is_none() {
        eprintln!(
            "skipping integration test because ACCESS_KEY_ID / ACCESS_KEY_SECRET / PROJECT / LOGSTORE / ENDPOINT are not fully configured"
        );
    }
    env
}

pub fn build_client(env: &TestEnvironment) -> Client {
    Client::from_config(build_config(env)).expect("client should build")
}

pub fn build_config(env: &TestEnvironment) -> Config {
    Config::builder()
        .access_key(&env.access_key_id, &env.access_key_secret)
        .endpoint(&env.endpoint)
        .build()
        .expect("valid client config")
}

pub fn producer_builder(env: &TestEnvironment) -> ProducerBuilder {
    Producer::builder()
        .access_key(&env.access_key_id, &env.access_key_secret)
        .endpoint(&env.endpoint)
        .project(&env.project)
        .logstore(&env.logstore)
        .export_timeout(Duration::from_secs(10))
        .concurrency(2)
}

pub fn unique_run_id(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("producer-it-{prefix}-{ts}")
}

pub fn make_record(run_id: &str, scenario: &str, seq: usize, payload_bytes: usize) -> LogRecord {
    let payload = "x".repeat(payload_bytes);
    LogRecord::new(SystemTime::now())
        .field("test_run_id", run_id)
        .field("scenario", scenario)
        .field("seq", seq.to_string())
        .field("record_id", format!("{run_id}-{scenario}-{seq}"))
        .field("payload", payload)
}

pub async fn wait_for_record_ids(
    client: &Client,
    env: &TestEnvironment,
    run_id: &str,
    topic: &str,
    since: SystemTime,
    expected: usize,
    timeout: Duration,
) -> BTreeSet<String> {
    let started = Instant::now();
    loop {
        let ids = fetch_record_ids_since(client, env, run_id, topic, since)
            .await
            .expect("should be able to fetch records from backend");
        if ids.len() >= expected {
            return ids;
        }
        assert!(
            started.elapsed() < timeout,
            "timed out waiting for {expected} records for run_id={run_id}, only saw {}",
            ids.len()
        );
        sleep(Duration::from_secs(1)).await;
    }
}

async fn fetch_record_ids_since(
    client: &Client,
    env: &TestEnvironment,
    run_id: &str,
    topic: &str,
    since: SystemTime,
) -> aliyun_log_rust_sdk::Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    let since_secs = since
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(2) as i64;

    let shards_response = client
        .list_shards(&env.project, &env.logstore)
        .send()
        .await?;
    let shards_body = shards_response.take_body();
    let shards = shards_body.shards();

    for shard in shards {
        let shard_id = *shard.shard_id();
        let start_cursor = client
            .get_cursor(&env.project, &env.logstore, shard_id)
            .cursor_pos(CursorPos::UnixTimeStamp(since_secs))
            .send()
            .await?
            .take_body()
            .cursor()
            .to_string();

        let end_cursor = client
            .get_cursor(&env.project, &env.logstore, shard_id)
            .cursor_pos(CursorPos::End)
            .send()
            .await?
            .take_body()
            .cursor()
            .to_string();

        let mut cursor = start_cursor;
        let mut loops = 0usize;
        while cursor != end_cursor && loops < 32 {
            loops += 1;
            let response = client
                .pull_logs(&env.project, &env.logstore, shard_id)
                .cursor(cursor.clone())
                .end_cursor(end_cursor.clone())
                .count(1000)
                .send()
                .await?;

            let body = response.take_body();
            let next_cursor = body.next_cursor().to_string();
            for group in body.log_group_list() {
                if group.topic().as_deref() != Some(topic) {
                    continue;
                }
                for log in group.logs() {
                    let fields = log_fields(log);
                    if fields.get("test_run_id").map(String::as_str) == Some(run_id) {
                        if let Some(record_id) = fields.get("record_id") {
                            ids.insert(record_id.clone());
                        }
                    }
                }
            }

            if next_cursor.is_empty() || next_cursor == cursor {
                break;
            }
            cursor = next_cursor;
        }
    }

    Ok(ids)
}

fn log_fields(log: &aliyun_log_sdk_protobuf::Log) -> BTreeMap<String, String> {
    log.contents()
        .iter()
        .map(|content| (content.key().clone(), content.value().clone()))
        .collect()
}

