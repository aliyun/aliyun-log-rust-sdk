use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use crate::Client;

use super::{ConsumerConfig, Error, Result};

#[derive(Default)]
struct State {
    pending_checkpoint: String,
    saved_checkpoint: String,
    last_current_cursor: String,
    last_next_cursor: String,
    generation: u64,
}

/// Tracks the cursors of one shard and commits processed progress to SLS.
///
/// Calling `save_checkpoint(false)` only marks the next cursor for the
/// periodic commit. Calling it with `true` sends the checkpoint immediately.
#[derive(Clone)]
pub struct CheckpointTracker {
    committer: CheckpointCommitter,
    current_cursor: String,
    next_cursor: String,
    deferred_saved: Arc<AtomicBool>,
    generation: u64,
}

#[derive(Clone)]
pub(crate) struct CheckpointCommitter {
    client: Arc<Client>,
    config: Arc<ConsumerConfig>,
    shard_id: i32,
    state: Arc<Mutex<State>>,
}

impl CheckpointCommitter {
    pub(crate) fn new(client: Arc<Client>, config: Arc<ConsumerConfig>, shard_id: i32) -> Self {
        Self {
            client,
            config,
            shard_id,
            state: Arc::new(Mutex::new(State::default())),
        }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn tracker(&self, current_cursor: String, next_cursor: String) -> CheckpointTracker {
        let generation = {
            let mut state = self.state();
            state.last_current_cursor.clone_from(&current_cursor);
            state.last_next_cursor.clone_from(&next_cursor);
            state.generation = state.generation.saturating_add(1);
            state.generation
        };
        CheckpointTracker {
            committer: self.clone(),
            current_cursor,
            next_cursor,
            deferred_saved: Arc::new(AtomicBool::new(false)),
            generation,
        }
    }

    pub(crate) fn last_tracker(&self) -> CheckpointTracker {
        let state = self.state();
        CheckpointTracker {
            committer: self.clone(),
            current_cursor: state.last_current_cursor.clone(),
            next_cursor: state.last_next_cursor.clone(),
            deferred_saved: Arc::new(AtomicBool::new(false)),
            generation: state.generation,
        }
    }

    pub(crate) fn initialize(&self, checkpoint: String) {
        self.state().saved_checkpoint = checkpoint;
    }

    pub(crate) async fn flush(&self) -> Result<()> {
        let (pending, saved) = {
            let state = self.state();
            (
                state.pending_checkpoint.clone(),
                state.saved_checkpoint.clone(),
            )
        };
        if pending.is_empty() || pending == saved {
            return Ok(());
        }

        let mut last_error = None;
        for attempt in 0..3 {
            match self
                .client
                .update_consumer_group_checkpoint(
                    &self.config.project,
                    &self.config.logstore,
                    &self.config.consumer_group,
                )
                .consumer_id(&self.config.consumer_name)
                .shard_id(self.shard_id)
                .checkpoint(&pending)
                .force_success(true)
                .send()
                .await
            {
                Ok(_) => {
                    self.mark_saved_if_pending(&pending);
                    return Ok(());
                }
                Err(error) if checkpoint_is_no_longer_owned(&error) => {
                    log::warn!(
                        "consumer no longer owns shard {}; dropping checkpoint update: {}",
                        self.shard_id,
                        error
                    );
                    self.mark_saved_if_pending(&pending);
                    return Ok(());
                }
                Err(error) => last_error = Some(error),
            }
            if attempt < 2 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        match last_error {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }

    fn mark_saved_if_pending(&self, checkpoint: &str) {
        let mut state = self.state();
        if state.pending_checkpoint == checkpoint {
            state.saved_checkpoint = checkpoint.to_string();
        }
    }
}

impl CheckpointTracker {
    pub fn shard_id(&self) -> i32 {
        self.committer.shard_id
    }

    pub fn checkpoint(&self) -> String {
        self.committer.state().saved_checkpoint.clone()
    }

    pub fn current_cursor(&self) -> String {
        self.current_cursor.clone()
    }

    pub fn next_cursor(&self) -> String {
        self.next_cursor.clone()
    }

    /// Mark the current batch as processed, optionally committing immediately.
    pub async fn save_checkpoint(&self, force: bool) -> Result<()> {
        if force {
            self.mark_pending()?;
            self.flush().await?;
        } else {
            self.ensure_current()?;
            self.deferred_saved.store(true, Ordering::Release);
        }
        Ok(())
    }

    pub(crate) fn commit_deferred(&self) -> Result<()> {
        if self.deferred_saved.swap(false, Ordering::AcqRel) {
            self.mark_pending()?;
        }
        Ok(())
    }

    fn ensure_current(&self) -> Result<()> {
        if self.committer.state().generation != self.generation {
            return Err(Error::StaleCheckpoint {
                shard_id: self.shard_id(),
            });
        }
        Ok(())
    }

    fn mark_pending(&self) -> Result<()> {
        let mut state = self.committer.state();
        if state.generation != self.generation {
            return Err(Error::StaleCheckpoint {
                shard_id: self.shard_id(),
            });
        }
        state.pending_checkpoint.clone_from(&self.next_cursor);
        Ok(())
    }

    pub(crate) async fn flush(&self) -> Result<()> {
        self.committer.flush().await
    }
}

fn checkpoint_is_no_longer_owned(error: &crate::Error) -> bool {
    match error {
        crate::Error::Server { error_code, .. } => matches!(
            error_code.to_ascii_lowercase().as_str(),
            "consumernotexsit"
                | "consumernotexist"
                | "consumernotmatch"
                | "shardnotexsit"
                | "shardnotexist"
        ),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, FromConfig};

    fn tracker() -> CheckpointTracker {
        let client = Client::from_config(
            Config::builder()
                .endpoint("localhost")
                .access_key("id", "secret")
                .build()
                .unwrap(),
        )
        .unwrap();
        let committer = CheckpointCommitter::new(
            Arc::new(client),
            Arc::new(ConsumerConfig::new("p", "l", "g", "c")),
            3,
        );
        committer.initialize("saved".into());
        committer.tracker("current".into(), "next".into())
    }

    #[tokio::test]
    async fn deferred_save_marks_next_cursor_without_network_io() {
        let tracker = tracker();

        tracker.save_checkpoint(false).await.unwrap();
        assert!(tracker.committer.state().pending_checkpoint.is_empty());
        tracker.commit_deferred().unwrap();

        assert_eq!(tracker.shard_id(), 3);
        assert_eq!(tracker.checkpoint(), "saved");
        assert_eq!(tracker.current_cursor(), "current");
        assert_eq!(tracker.next_cursor(), "next");
        assert_eq!(tracker.committer.state().pending_checkpoint, "next");
    }

    #[tokio::test]
    async fn stale_tracker_cannot_advance_a_newer_batch() {
        let first = tracker();
        let committer = first.committer.clone();
        let _second = committer.tracker("next".into(), "after-next".into());

        assert!(matches!(
            first.save_checkpoint(false).await,
            Err(Error::StaleCheckpoint { shard_id: 3 })
        ));

        assert!(committer.state().pending_checkpoint.is_empty());
    }
}
