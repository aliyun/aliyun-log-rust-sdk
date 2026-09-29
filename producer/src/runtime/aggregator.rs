use std::{
    collections::{hash_map::Entry, HashMap},
    sync::Arc,
    time::Instant,
};

use tokio::{
    sync::{mpsc, watch},
    time::{interval, Interval, MissedTickBehavior},
};

use crate::{
    batch::{Batch, Command, Envelope, Key, PreparedLog},
    state::Shared,
    ProducerConfig, ProducerState,
};

/// Owns all active batches. Moved into one task, so batch mutation needs no locks.
pub(super) struct Aggregator {
    shared: Arc<Shared>,
    input: mpsc::Receiver<Command>,
    output: BatchOutput,
    batches: HashMap<Key, BatchSlot>,
    empty_slots: usize,
    tick: Interval,
    changes: watch::Receiver<()>,
}

impl Aggregator {
    pub(super) fn new(
        shared: Arc<Shared>,
        input: mpsc::Receiver<Command>,
        output: flume::Sender<Batch>,
    ) -> Self {
        let mut tick = interval(shared.runtime_config.tick_interval);
        tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let changes = shared.changes.subscribe();
        Self {
            output: BatchOutput {
                pressure: PressureCheck::new(&shared.config),
                shared: shared.clone(),
                tx: output,
                pack_ids: HashMap::new(),
            },
            shared,
            input,
            batches: HashMap::new(),
            empty_slots: 0,
            tick,
            changes,
        }
    }

    pub(super) async fn run(mut self) -> Result<(), String> {
        loop {
            if self.shared.gate.lock().unwrap().state != ProducerState::Running {
                self.input.close();
            }
            tokio::select! {
                _ = self.changes.changed() => {}
                _ = self.tick.tick() => {
                    self.output.pressure.refresh(&self.shared);
                    self.flush_due().await?;
                }
                command = self.input.recv() => {
                    let Some(command) = command else { return self.flush_all().await; };
                    self.process_command(command).await?;
                    let mut consumed = 1;
                    while consumed < 64 {
                        match self.input.try_recv() {
                            Ok(command) => self.process_command(command).await?,
                            Err(mpsc::error::TryRecvError::Empty) => break,
                            Err(mpsc::error::TryRecvError::Disconnected) => return self.flush_all().await,
                        }
                        consumed += 1;
                    }
                    if consumed == 64 {
                        tokio::task::yield_now().await;
                    }
                }
            }
        }
    }

    // Process commands in order without collecting another Vec or waiting to fill a chunk.
    async fn process_command(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Log(envelope) => {
                self.output
                    .pressure
                    .on_submission(&self.shared, envelope.submission.raw_bytes);
                self.append(envelope).await
            }
            Command::Flush => self.flush_all().await,
        }
    }

    async fn append(&mut self, envelope: Envelope) -> Result<(), String> {
        let Envelope {
            key,
            submission,
            log,
        } = envelope;
        let PreparedLog { log, size } = log;
        let slot = match self.batches.entry(key) {
            Entry::Occupied(entry) => {
                let slot = entry.into_mut();
                if slot.current.is_none() {
                    self.empty_slots -= 1;
                }
                slot
            }
            Entry::Vacant(entry) => {
                let key = entry.key().clone();
                entry.insert(BatchSlot { key, current: None })
            }
        };
        let config = &self.shared.config;
        if slot.would_overflow(size) {
            let batch = slot.current.take().expect("nonempty batch exceeds limit");
            self.output.forward(batch, &mut self.tick).await?;
        }
        // Move the inline log directly into the final batch, with no temporary Vec.
        let batch = slot
            .current
            .get_or_insert_with(|| Batch::new(slot.key.clone(), submission.admitted));
        batch.push_one(log, size, submission);
        if slot.is_full(config) {
            let batch = slot.current.take().expect("full batch");
            self.output.forward(batch, &mut self.tick).await?;
        }
        if slot.current.is_none() {
            self.empty_slots += 1;
        }
        self.prune_empty_slots();
        Ok(())
    }

    async fn flush_due(&mut self) -> Result<(), String> {
        let now = Instant::now();
        for slot in self.batches.values_mut() {
            if slot
                .current
                .as_ref()
                .is_some_and(|batch| now.duration_since(batch.oldest) >= self.shared.config.linger)
            {
                let batch = slot.current.take().expect("due batch");
                self.empty_slots += 1;
                self.output.forward(batch, &mut self.tick).await?;
            }
        }
        self.prune_empty_slots();
        Ok(())
    }

    async fn flush_all(&mut self) -> Result<(), String> {
        for slot in self.batches.values_mut() {
            if let Some(batch) = slot.current.take() {
                self.empty_slots += 1;
                self.output.forward(batch, &mut self.tick).await?;
            }
        }
        self.prune_empty_slots();
        Ok(())
    }

    fn prune_empty_slots(&mut self) {
        // Retain common keys across flushes without retaining unbounded historical
        // source/topic values. Prune in batches to avoid scanning the whole map
        // for each new key near the limit. Active batches are never evicted.
        const MAX_EMPTY_SLOTS: usize = 128;
        const RETAIN_EMPTY_SLOTS: usize = 64;
        if self.empty_slots <= MAX_EMPTY_SLOTS {
            return;
        }
        let mut excess = self.empty_slots - RETAIN_EMPTY_SLOTS;
        self.batches.retain(|_, slot| {
            if excess != 0 && slot.current.is_none() {
                excess -= 1;
                false
            } else {
                true
            }
        });
        self.empty_slots = RETAIN_EMPTY_SLOTS;
    }
}

/// Persistent aggregation identity; only the payload leaves when a batch seals.
struct BatchSlot {
    key: Key,
    current: Option<Batch>,
}

impl BatchSlot {
    fn would_overflow(&self, size: usize) -> bool {
        self.current.as_ref().is_some_and(|batch| {
            batch.size.saturating_add(size) > crate::config::MAX_BATCH_BYTES
                || batch.group.logs().len() + 1 > crate::config::MAX_BATCH_LOGS
        })
    }

    fn is_full(&self, config: &ProducerConfig) -> bool {
        self.current.as_ref().is_some_and(|batch| {
            config.linger.is_zero()
                || batch.size >= config.batch_size_threshold
                || batch.group.logs().len() >= config.batch_count_threshold
        })
    }
}

/// Forwarding borrows only the output state, leaving map slots in place even
/// while the bounded downstream queue is full.
struct BatchOutput {
    shared: Arc<Shared>,
    pressure: PressureCheck,
    tx: flume::Sender<Batch>,
    pack_ids: HashMap<usize, crate::pack_id::PackIdGenerator>,
}

impl BatchOutput {
    async fn forward(&mut self, mut batch: Batch, tick: &mut Interval) -> Result<(), String> {
        batch.sealed_at = Some(Instant::now());
        if self.shared.config.generate_pack_id {
            let id = self
                .pack_ids
                .entry(batch.key.logstore)
                .or_insert_with(crate::pack_id::PackIdGenerator::new)
                .next();
            batch
                .group
                .add_log_tag_kv(crate::pack_id::PACK_ID_TAG, id.clone());
            batch.pack_id = Some(id.into());
        }
        let send = self.tx.send_async(batch);
        tokio::pin!(send);
        loop {
            tokio::select! {
                result = &mut send => return result.map_err(|_| "batch queue disconnected".to_owned()),
                _ = tick.tick() => self.pressure.refresh(&self.shared),
            }
        }
    }
}

/// Local to the aggregator, including waits for downstream capacity. Low-pressure
/// submissions only decrement a byte allowance; they do not read shared counters.
struct PressureCheck {
    interval_bytes: usize,
    remaining_bytes: usize,
}

impl PressureCheck {
    fn new(config: &ProducerConfig) -> Self {
        Self {
            interval_bytes: (config.buffer_bytes / 8).clamp(1, 1024 * 1024),
            remaining_bytes: 0, // Sample the first submission.
        }
    }

    fn on_submission(&mut self, shared: &Shared, raw_bytes: usize) {
        if raw_bytes < self.remaining_bytes {
            self.remaining_bytes -= raw_bytes;
        } else {
            self.refresh(shared);
        }
    }

    fn refresh(&mut self, shared: &Shared) {
        let bytes = shared.refresh_pressure();
        self.remaining_bytes = if bytes < shared.config.buffer_bytes / 2 {
            self.interval_bytes
        } else {
            0 // Near the limit, sample every submission until pressure subsides.
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{batch, state::Submission, Log, SubmissionId};
    use std::time::Duration;

    #[test]
    fn pressure_sampling_skips_small_submissions_but_checks_large_ones_and_recovers() {
        use std::sync::atomic::Ordering::Relaxed;
        let shared = Shared::for_test(
            0,
            ProducerConfig::default().with_buffer_bytes(1024),
            [("project", "store-0")],
        );
        let mut check = PressureCheck::new(&shared.config);
        check.refresh(&shared);
        shared.raw_bytes.store(1024, Relaxed);
        check.on_submission(&shared, 127);
        assert!(
            !shared.overloaded.load(Relaxed),
            "low-pressure sample was not cached"
        );
        check.on_submission(&shared, 1);
        assert!(shared.overloaded.load(Relaxed));

        shared.raw_bytes.store(900, Relaxed);
        check.on_submission(&shared, 1);
        assert!(
            shared.overloaded.load(Relaxed),
            "hysteresis must retain pressure"
        );
        shared.raw_bytes.store(800, Relaxed);
        check.on_submission(&shared, 1);
        assert!(!shared.overloaded.load(Relaxed));
        shared.raw_bytes.store(1024, Relaxed);
        check.on_submission(&shared, 1);
        assert!(
            shared.overloaded.load(Relaxed),
            "above 50% must sample each submission"
        );

        shared.raw_bytes.store(0, Relaxed);
        // The timer clears pressure even without input, allowing subsequent sends.
        check.refresh(&shared);
        assert!(!shared.overloaded.load(Relaxed));
        shared.raw_bytes.store(1024, Relaxed);
        check.on_submission(&shared, 1024);
        assert!(
            shared.overloaded.load(Relaxed),
            "large submissions must not be skipped"
        );
    }

    fn setup() -> (Aggregator, flume::Receiver<Batch>) {
        let config = ProducerConfig::default()
            .with_batch_count_threshold(2)
            .with_linger(Duration::from_secs(60));
        let shared = Shared::for_test(0, config, [("project", "store-0")]);
        let (_, input) = mpsc::channel(1);
        let (output, batches) = flume::bounded(1);
        (Aggregator::new(shared, input, output), batches)
    }

    fn envelope(source: &str, id: u64) -> Envelope {
        let mut log = Log::from_unixtime(1);
        log.add_content_kv("message", id.to_string());
        let (size, bytes) = batch::measure_log(&log);
        let key = Key {
            logstore: 0,
            source: source.to_owned(),
            topic: "topic".into(),
        };
        let submission = Submission::new(SubmissionId(id), bytes);
        Envelope {
            key,
            submission,
            log: PreparedLog { log, size },
        }
    }

    #[tokio::test]
    async fn retained_slot_resets_linger_and_preserves_pack_ids_across_flushes() {
        let (mut aggregator, batches) = setup();
        let first = envelope("host", 1);
        let key = first.key.clone();
        aggregator.append(first).await.unwrap();
        aggregator
            .batches
            .get_mut(&key)
            .unwrap()
            .current
            .as_mut()
            .unwrap()
            .oldest = Instant::now() - Duration::from_secs(120);
        aggregator.flush_due().await.unwrap();
        let expired = batches.try_recv().unwrap();
        assert_eq!(expired.group.logs().len(), 1);
        assert_eq!(aggregator.empty_slots, 1);
        assert!(aggregator.batches[&key].current.is_none());

        let next = envelope("host", 2);
        let admitted = next.submission.admitted;
        aggregator.append(next).await.unwrap();
        assert_eq!(
            aggregator.batches[&key].current.as_ref().unwrap().oldest,
            admitted
        );
        aggregator.flush_due().await.unwrap();
        assert!(
            batches.try_recv().is_err(),
            "new batch inherited the expired deadline"
        );
        assert_eq!(aggregator.empty_slots, 0);

        let last = envelope("host", 3);
        aggregator.append(last).await.unwrap();
        let full = batches.try_recv().unwrap();
        assert_eq!(full.group.logs().len(), 2);
        assert_eq!(full.submission_ids, [SubmissionId(2), SubmissionId(3)]);
        assert_eq!(full.key, key);
        assert_ne!(full.pack_id, expired.pack_id);
        assert!(full.pack_id.is_some());
        assert_eq!(aggregator.batches.len(), 1);
        assert_eq!(aggregator.empty_slots, 1);
        aggregator.flush_all().await.unwrap();
        assert!(
            batches.try_recv().is_err(),
            "flush emitted an empty retained slot"
        );

        let final_log = envelope("host", 4);
        aggregator.append(final_log).await.unwrap();
        aggregator.flush_all().await.unwrap();
        assert_eq!(batches.try_recv().unwrap().group.logs().len(), 1);
        assert_eq!(aggregator.batches.len(), 1);
        assert_eq!(aggregator.empty_slots, 1);
    }

    #[tokio::test]
    async fn historical_key_cache_is_bounded_without_evicting_active_batches() {
        let (mut aggregator, batches) = setup();
        let active = envelope("active", 1);
        let active_key = active.key.clone();
        aggregator.append(active).await.unwrap();
        let mut prunes = 0;
        for id in 2..302 {
            let previous_empty = aggregator.empty_slots;
            for offset in 0..2 {
                let log = envelope(&format!("host-{id}"), id * 2 + offset);
                aggregator.append(log).await.unwrap();
            }
            assert_eq!(batches.try_recv().unwrap().group.logs().len(), 2);
            if aggregator.empty_slots < previous_empty {
                prunes += 1;
                assert!(previous_empty - aggregator.empty_slots >= 64);
            }
            assert!(aggregator.empty_slots <= 128);
            assert!(aggregator.batches.len() <= 129);
            assert_eq!(
                aggregator.batches[&active_key]
                    .current
                    .as_ref()
                    .unwrap()
                    .group
                    .logs()
                    .len(),
                1
            );
        }
        assert!(
            prunes > 0 && prunes < 10,
            "prune in batches, not once per new key"
        );
        aggregator.flush_all().await.unwrap();
        assert_eq!(batches.try_recv().unwrap().key, active_key);
        assert!(aggregator.batches.len() <= 128);
        assert_eq!(aggregator.empty_slots, aggregator.batches.len());
    }
}
