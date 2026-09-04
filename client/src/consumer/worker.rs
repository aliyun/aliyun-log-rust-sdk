use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures_util::FutureExt;
use tokio::sync::{Notify, Semaphore};
use tokio::task::JoinHandle;

use crate::get_cursor_models::CursorPos;
use crate::Client;

use super::{
    CheckpointCommitter, CheckpointTracker, ConsumerConfig, CursorPosition, Error, LogGroupBatch,
    ProcessOutcome, Processor, Result,
};

struct ShardTask {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Result<()>>,
}

/// Coordinates heartbeats and one asynchronous consumer task per assigned shard.
pub struct ConsumerWorker<P: Processor> {
    client: Arc<Client>,
    config: Arc<ConsumerConfig>,
    processor: Arc<P>,
    stop: Arc<AtomicBool>,
    wake: Arc<Notify>,
    task: Option<JoinHandle<Result<()>>>,
}

impl<P: Processor> ConsumerWorker<P> {
    pub fn new(client: Client, config: ConsumerConfig, processor: P) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            client: Arc::new(client),
            config: Arc::new(config),
            processor: Arc::new(processor),
            stop: Arc::new(AtomicBool::new(false)),
            wake: Arc::new(Notify::new()),
            task: None,
        })
    }

    /// Create or update the remote consumer group and start consuming.
    pub async fn start(&mut self) -> Result<()> {
        if self.task.is_some() {
            return Err(Error::AlreadyStarted);
        }
        ensure_consumer_group(&self.client, &self.config).await?;
        self.stop.store(false, Ordering::Release);

        let client = Arc::clone(&self.client);
        let config = Arc::clone(&self.config);
        let processor = Arc::clone(&self.processor);
        let stop = Arc::clone(&self.stop);
        let wake = Arc::clone(&self.wake);
        self.task = Some(tokio::spawn(coordinate(
            client, config, processor, stop, wake,
        )));
        Ok(())
    }

    /// Signal all tasks to stop, invoke processor shutdown hooks, and flush checkpoints.
    pub async fn stop_and_wait(&mut self) -> Result<()> {
        self.stop();
        self.wait().await
    }

    /// Signal the worker to stop without waiting for shutdown to finish.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Release);
        self.wake.notify_waiters();
    }

    /// Wait for the worker to stop and surface heartbeat, processor, or task failures.
    pub async fn wait(&mut self) -> Result<()> {
        if let Some(task) = self.task.take() {
            task.await??;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.task.as_ref().is_some_and(|task| !task.is_finished())
    }
}

impl<P: Processor> Drop for ConsumerWorker<P> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.wake.notify_waiters();
    }
}

async fn ensure_consumer_group(client: &Client, config: &ConsumerConfig) -> Result<()> {
    let response = client
        .list_consumer_groups(&config.project, &config.logstore)
        .send()
        .await?;
    let exists = response
        .get_body()
        .consumer_groups()
        .iter()
        .any(|group| group.consumer_group_name() == &config.consumer_group);
    let timeout = i32::try_from(config.heartbeat_timeout.as_secs()).map_err(|_| {
        Error::InvalidConfig("heartbeat_timeout is too large for the SLS API".into())
    })?;

    if exists {
        client
            .update_consumer_group(&config.project, &config.logstore, &config.consumer_group)
            .timeout(timeout)
            .order(config.in_order)
            .send()
            .await?;
    } else {
        let result = client
            .create_consumer_group(&config.project, &config.logstore, &config.consumer_group)
            .timeout(timeout)
            .order(config.in_order)
            .send()
            .await;
        if let Err(error) = result {
            let raced_with_another_creator = matches!(
                &error,
                crate::Error::Server { error_code, .. }
                    if error_code.eq_ignore_ascii_case("ConsumerGroupAlreadyExist")
            );
            if !raced_with_another_creator {
                return Err(error.into());
            }
        }
    }
    Ok(())
}

async fn coordinate<P: Processor>(
    client: Arc<Client>,
    config: Arc<ConsumerConfig>,
    processor: Arc<P>,
    stop: Arc<AtomicBool>,
    wake: Arc<Notify>,
) -> Result<()> {
    let io_limit = Arc::new(Semaphore::new(config.max_io_workers));
    let mut tasks: HashMap<i32, ShardTask> = HashMap::new();
    let mut last_heartbeat_success = Instant::now();

    let failure = loop {
        if stop.load(Ordering::Acquire) {
            break None;
        }
        if let Err(error) = reap_finished(&mut tasks).await {
            break Some(error);
        }
        let reported_shards = tasks.keys().copied().collect::<Vec<_>>();
        let assigned = client
            .consumer_group_heartbeat(&config.project, &config.logstore, &config.consumer_group)
            .consumer(&config.consumer_name)
            .shards(reported_shards)
            .send()
            .await;

        match assigned {
            Ok(response) => {
                last_heartbeat_success = Instant::now();
                reconcile(
                    response.get_body().shards(),
                    &mut tasks,
                    &client,
                    &config,
                    &processor,
                    &io_limit,
                );
            }
            Err(error) => {
                log::warn!("consumer heartbeat failed: {error}");
                if last_heartbeat_success.elapsed()
                    > config.heartbeat_timeout + config.heartbeat_interval
                {
                    break Some(Error::Heartbeat(Box::new(error)));
                }
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(config.heartbeat_interval) => {},
            _ = wake.notified() => {},
        }
    };

    for task in tasks.values() {
        task.stop.store(true, Ordering::Release);
    }
    let mut shutdown_failure = None;
    for (_, task) in tasks {
        match task.handle.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                shutdown_failure.get_or_insert(error);
            }
            Err(error) => {
                shutdown_failure.get_or_insert(Error::Task(error));
            }
        }
    }
    match failure.or(shutdown_failure) {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn reconcile<P: Processor>(
    assigned: &[i32],
    tasks: &mut HashMap<i32, ShardTask>,
    client: &Arc<Client>,
    config: &Arc<ConsumerConfig>,
    processor: &Arc<P>,
    io_limit: &Arc<Semaphore>,
) {
    for (&shard, task) in tasks.iter() {
        if !assigned.contains(&shard) {
            task.stop.store(true, Ordering::Release);
        }
    }
    for &shard in assigned {
        if tasks.contains_key(&shard) {
            continue;
        }
        let shard_stop = Arc::new(AtomicBool::new(false));
        let handle = tokio::spawn(consume_shard(
            shard,
            Arc::clone(client),
            Arc::clone(config),
            Arc::clone(processor),
            Arc::clone(&shard_stop),
            Arc::clone(io_limit),
        ));
        tasks.insert(
            shard,
            ShardTask {
                stop: shard_stop,
                handle,
            },
        );
    }
}

async fn reap_finished(tasks: &mut HashMap<i32, ShardTask>) -> Result<()> {
    let finished = tasks
        .iter()
        .filter_map(|(&shard, task)| task.handle.is_finished().then_some(shard))
        .collect::<Vec<_>>();
    for shard in finished {
        if let Some(task) = tasks.remove(&shard) {
            task.handle.await??;
        }
    }
    Ok(())
}

async fn consume_shard<P: Processor>(
    shard: i32,
    client: Arc<Client>,
    config: Arc<ConsumerConfig>,
    processor: Arc<P>,
    stop: Arc<AtomicBool>,
    io_limit: Arc<Semaphore>,
) -> Result<()> {
    let checkpoint = CheckpointCommitter::new(Arc::clone(&client), Arc::clone(&config), shard);
    let work = std::panic::AssertUnwindSafe(consume_shard_loop(
        shard,
        client,
        Arc::clone(&config),
        Arc::clone(&processor),
        stop,
        io_limit,
        checkpoint.clone(),
    ))
    .catch_unwind()
    .await;
    let shutdown = shutdown_shard(
        &*processor,
        checkpoint.last_tracker(),
        config.shutdown_timeout,
    )
    .await;
    match work {
        Ok(result) => result?,
        Err(_) => return Err(Error::ProcessorPanicked { shard_id: shard }),
    }
    shutdown
}

#[allow(clippy::too_many_arguments)]
async fn consume_shard_loop<P: Processor>(
    shard: i32,
    client: Arc<Client>,
    config: Arc<ConsumerConfig>,
    processor: Arc<P>,
    stop: Arc<AtomicBool>,
    io_limit: Arc<Semaphore>,
    checkpoint: CheckpointCommitter,
) -> Result<()> {
    let (mut cursor, end_cursor) = loop {
        if stop.load(Ordering::Acquire) {
            return Ok(());
        }
        match initial_cursors(&client, &config, shard, &checkpoint).await {
            Ok(cursors) => break cursors,
            Err(error) => {
                log::warn!("failed to initialize shard {shard}: {error}");
                sleep_unless_stopped(Duration::from_millis(100), &stop).await;
            }
        }
    };
    let mut last_checkpoint_flush = Instant::now();

    while !stop.load(Ordering::Acquire) {
        let fetch_started = Instant::now();
        let permit = match Arc::clone(&io_limit).acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => break,
        };
        let mut request = client
            .pull_logs(&config.project, &config.logstore, shard)
            .cursor(&cursor)
            .count(config.max_fetch_log_group_count);
        if let Some(value) = &end_cursor {
            request = request.end_cursor(value);
        }
        if let Some(value) = &config.query {
            request = request.query(value);
        }
        if let Some(value) = &config.processor {
            request = request.query_id(value);
        }
        let response = request.send().await;
        drop(permit);

        let response = match response {
            Ok(response) => response.take_body(),
            Err(error) => {
                log::warn!("failed to pull shard {shard} at cursor {cursor}: {error}");
                sleep_unless_stopped(Duration::from_millis(100), &stop).await;
                continue;
            }
        };
        let next_cursor = response.next_cursor().clone();
        let group_count = *response.log_group_count();
        let batch_checkpoint = checkpoint.tracker(cursor.clone(), next_cursor.clone());
        let log_groups: LogGroupBatch = Arc::from(response.into_log_group_list());

        let mut processor_failures = 0;
        let next = loop {
            match processor
                .process(shard, log_groups.clone(), batch_checkpoint.clone())
                .await
            {
                Ok(ProcessOutcome::Continue) => {
                    batch_checkpoint.commit_deferred()?;
                    break next_cursor.clone();
                }
                Ok(ProcessOutcome::Rollback(value)) => break value,
                Err(error) => {
                    processor_failures += 1;
                    log::error!("processor failed for shard {shard}; retrying batch: {error}");
                    if config
                        .processor_retry_limit
                        .is_some_and(|limit| processor_failures >= limit)
                    {
                        return Err(Error::Processor {
                            shard_id: shard,
                            source: error,
                        });
                    }
                    if stop.load(Ordering::Acquire) {
                        break next_cursor.clone();
                    }
                    sleep_unless_stopped(config.processor_retry_interval, &stop).await;
                }
            }
        };
        cursor = next;

        if config.auto_commit && last_checkpoint_flush.elapsed() >= config.auto_commit_interval {
            if let Err(error) = checkpoint.flush().await {
                log::error!("failed to flush checkpoint for shard {shard}: {error}");
            }
            last_checkpoint_flush = Instant::now();
        }
        if cursor == next_cursor && batch_checkpoint.current_cursor() == next_cursor {
            sleep_unless_stopped(Duration::from_millis(500), &stop).await;
        } else if end_cursor.as_deref() == Some(cursor.as_str()) {
            sleep_unless_stopped(Duration::from_secs(5), &stop).await;
        } else {
            let target = adaptive_fetch_interval(&config, group_count);
            if let Some(remaining) = target.checked_sub(fetch_started.elapsed()) {
                sleep_unless_stopped(remaining, &stop).await;
            }
        }
    }
    Ok(())
}

async fn initial_cursors(
    client: &Client,
    config: &ConsumerConfig,
    shard: i32,
    checkpoint: &CheckpointCommitter,
) -> Result<(String, Option<String>)> {
    let end_cursor = if let Some(time) = config.cursor_end_time {
        Some(
            client
                .get_cursor(&config.project, &config.logstore, shard)
                .cursor_pos(CursorPos::UnixTimeStamp(time))
                .send()
                .await?
                .get_body()
                .cursor()
                .to_string(),
        )
    } else {
        None
    };

    let checkpoints = client
        .get_consumer_group_checkpoint(&config.project, &config.logstore, &config.consumer_group)
        .shard_id(shard)
        .send()
        .await?;
    if let Some(saved) = checkpoints
        .get_body()
        .checkpoints()
        .iter()
        .find(|value| *value.shard_id() == shard)
        .map(|value| value.checkpoint().clone())
        .filter(|value| !value.is_empty())
    {
        checkpoint.initialize(saved.clone());
        return Ok((saved, end_cursor));
    }

    let position = match config.cursor_position {
        CursorPosition::Begin => CursorPos::Begin,
        CursorPosition::End => CursorPos::End,
        CursorPosition::At(time) => CursorPos::UnixTimeStamp(time),
    };
    let cursor = client
        .get_cursor(&config.project, &config.logstore, shard)
        .cursor_pos(position)
        .send()
        .await?
        .get_body()
        .cursor()
        .to_string();
    Ok((cursor, end_cursor))
}

async fn shutdown_shard<P: Processor>(
    processor: &P,
    checkpoint: CheckpointTracker,
    timeout: Duration,
) -> Result<()> {
    let shard_id = checkpoint.shard_id();
    let shutdown = async {
        loop {
            match processor.shutdown(checkpoint.clone()).await {
                Ok(()) => break,
                Err(error) => {
                    log::error!(
                        "processor shutdown failed for shard {shard_id}; retrying: {error}"
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        checkpoint.commit_deferred()?;
        loop {
            match checkpoint.flush().await {
                Ok(()) => break,
                Err(error) => {
                    log::error!(
                        "final checkpoint flush failed for shard {shard_id}; retrying: {error}"
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
        Ok::<(), Error>(())
    };
    tokio::time::timeout(timeout, shutdown)
        .await
        .map_err(|_| Error::ShutdownTimeout { shard_id, timeout })??;
    Ok(())
}

fn adaptive_fetch_interval(config: &ConsumerConfig, group_count: i32) -> Duration {
    if group_count >= config.max_fetch_log_group_count {
        Duration::ZERO
    } else if group_count < 100 {
        config.data_fetch_interval.max(Duration::from_millis(500))
    } else if group_count < 500 {
        config.data_fetch_interval.max(Duration::from_millis(200))
    } else {
        config.data_fetch_interval.max(Duration::from_millis(50))
    }
}

async fn sleep_unless_stopped(duration: Duration, stop: &AtomicBool) {
    let started = Instant::now();
    while !stop.load(Ordering::Acquire) {
        let Some(remaining) = duration.checked_sub(started.elapsed()) else {
            break;
        };
        tokio::time::sleep(remaining.min(Duration::from_millis(100))).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, FromConfig};

    #[derive(Debug)]
    struct ShutdownFailure;

    impl std::fmt::Display for ShutdownFailure {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("shutdown failed")
        }
    }

    impl std::error::Error for ShutdownFailure {}

    struct FailingShutdown;

    impl Processor for FailingShutdown {
        fn process<'a>(
            &'a self,
            _shard_id: i32,
            _log_groups: LogGroupBatch,
            _checkpoint: CheckpointTracker,
        ) -> super::super::ProcessorFuture<'a> {
            Box::pin(async { Ok(ProcessOutcome::Continue) })
        }

        fn shutdown<'a>(
            &'a self,
            _checkpoint: CheckpointTracker,
        ) -> super::super::ProcessorFuture<'a, ()> {
            Box::pin(async { Err(Box::new(ShutdownFailure) as super::super::BoxProcessorError) })
        }
    }

    #[test]
    fn fetch_interval_adapts_to_batch_size() {
        let config = ConsumerConfig::new("p", "l", "g", "c");
        assert_eq!(
            adaptive_fetch_interval(&config, 10),
            Duration::from_millis(500)
        );
        assert_eq!(
            adaptive_fetch_interval(&config, 200),
            Duration::from_millis(200)
        );
        assert_eq!(adaptive_fetch_interval(&config, 1000), Duration::ZERO);
    }

    #[tokio::test]
    async fn shutdown_has_a_deadline() {
        let client = Client::from_config(
            Config::builder()
                .endpoint("localhost")
                .access_key("id", "secret")
                .build()
                .unwrap(),
        )
        .unwrap();
        let config = Arc::new(ConsumerConfig::new("p", "l", "g", "c"));
        let checkpoint = CheckpointCommitter::new(Arc::new(client), config, 7)
            .tracker("current".into(), "next".into());

        let result = shutdown_shard(&FailingShutdown, checkpoint, Duration::from_millis(5)).await;

        assert!(matches!(
            result,
            Err(Error::ShutdownTimeout { shard_id: 7, .. })
        ));
    }
}
