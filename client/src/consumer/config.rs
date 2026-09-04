use std::time::Duration;

use super::{Error, Result};

/// Position used for a shard that does not have a saved checkpoint yet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CursorPosition {
    /// Start at the oldest available log.
    Begin,
    /// Start at logs written after the consumer starts.
    #[default]
    End,
    /// Start at the given Unix timestamp (seconds).
    At(i64),
}

/// Configuration for a coordinated consumer.
#[derive(Clone, Debug)]
pub struct ConsumerConfig {
    pub(crate) project: String,
    pub(crate) logstore: String,
    pub(crate) consumer_group: String,
    pub(crate) consumer_name: String,
    pub(crate) cursor_position: CursorPosition,
    pub(crate) cursor_end_time: Option<i64>,
    pub(crate) heartbeat_interval: Duration,
    pub(crate) heartbeat_timeout: Duration,
    pub(crate) data_fetch_interval: Duration,
    pub(crate) max_fetch_log_group_count: i32,
    pub(crate) auto_commit: bool,
    pub(crate) auto_commit_interval: Duration,
    pub(crate) in_order: bool,
    pub(crate) query: Option<String>,
    pub(crate) processor: Option<String>,
    pub(crate) max_io_workers: usize,
    pub(crate) shutdown_timeout: Duration,
    pub(crate) processor_retry_limit: Option<u32>,
    pub(crate) processor_retry_interval: Duration,
}

impl ConsumerConfig {
    /// Create a configuration with Go-compatible polling defaults and bounded
    /// processor/shutdown failure handling.
    pub fn new(
        project: impl Into<String>,
        logstore: impl Into<String>,
        consumer_group: impl Into<String>,
        consumer_name: impl Into<String>,
    ) -> Self {
        Self {
            project: project.into(),
            logstore: logstore.into(),
            consumer_group: consumer_group.into(),
            consumer_name: consumer_name.into(),
            cursor_position: CursorPosition::End,
            cursor_end_time: None,
            heartbeat_interval: Duration::from_secs(20),
            heartbeat_timeout: Duration::from_secs(60),
            data_fetch_interval: Duration::from_millis(200),
            max_fetch_log_group_count: 1000,
            auto_commit: true,
            auto_commit_interval: Duration::from_secs(60),
            in_order: false,
            query: None,
            processor: None,
            max_io_workers: 50,
            shutdown_timeout: Duration::from_secs(30),
            processor_retry_limit: Some(10),
            processor_retry_interval: Duration::from_secs(1),
        }
    }

    pub fn cursor_position(mut self, value: CursorPosition) -> Self {
        self.cursor_position = value;
        self
    }

    /// Stop fetching new data after this Unix timestamp (seconds).
    pub fn cursor_end_time(mut self, value: i64) -> Self {
        self.cursor_end_time = (value != 0).then_some(value);
        self
    }

    pub fn heartbeat_interval(mut self, value: Duration) -> Self {
        self.heartbeat_interval = value;
        self
    }

    pub fn heartbeat_timeout(mut self, value: Duration) -> Self {
        self.heartbeat_timeout = value;
        self
    }

    pub fn data_fetch_interval(mut self, value: Duration) -> Self {
        self.data_fetch_interval = value;
        self
    }

    pub fn max_fetch_log_group_count(mut self, value: i32) -> Self {
        self.max_fetch_log_group_count = value;
        self
    }

    /// Enable or disable periodic flushing of checkpoints marked by processors.
    pub fn auto_commit(mut self, value: bool) -> Self {
        self.auto_commit = value;
        self
    }

    pub fn auto_commit_interval(mut self, value: Duration) -> Self {
        self.auto_commit_interval = value;
        self
    }

    pub fn in_order(mut self, value: bool) -> Self {
        self.in_order = value;
        self
    }

    pub fn query(mut self, value: impl Into<String>) -> Self {
        self.query = Some(value.into());
        self
    }

    /// Set the server-side consume processor (the pull API `queryId`).
    pub fn processor(mut self, value: impl Into<String>) -> Self {
        self.processor = Some(value.into());
        self
    }

    pub fn max_io_workers(mut self, value: usize) -> Self {
        self.max_io_workers = value;
        self
    }

    /// Limit consecutive processor failures for one batch.
    ///
    /// Pass `None` to retain the Go consumer's retry-forever behavior.
    pub fn processor_retry_limit(mut self, value: Option<u32>) -> Self {
        self.processor_retry_limit = value;
        self
    }

    pub fn processor_retry_interval(mut self, value: Duration) -> Self {
        self.processor_retry_interval = value;
        self
    }

    /// Maximum time spent running shutdown hooks and flushing a final checkpoint.
    pub fn shutdown_timeout(mut self, value: Duration) -> Self {
        self.shutdown_timeout = value;
        self
    }

    pub(crate) fn validate(&self) -> Result<()> {
        for (name, value) in [
            ("project", &self.project),
            ("logstore", &self.logstore),
            ("consumer_group", &self.consumer_group),
            ("consumer_name", &self.consumer_name),
        ] {
            if value.trim().is_empty() {
                return Err(Error::InvalidConfig(format!("{name} must not be empty")));
            }
        }
        if self.heartbeat_interval.is_zero() {
            return Err(Error::InvalidConfig(
                "heartbeat_interval must be greater than zero".into(),
            ));
        }
        if self.heartbeat_timeout < self.heartbeat_interval {
            return Err(Error::InvalidConfig(
                "heartbeat_timeout must be at least heartbeat_interval".into(),
            ));
        }
        if self.max_fetch_log_group_count <= 0 || self.max_fetch_log_group_count > 1000 {
            return Err(Error::InvalidConfig(
                "max_fetch_log_group_count must be in 1..=1000".into(),
            ));
        }
        if self.max_io_workers == 0 {
            return Err(Error::InvalidConfig(
                "max_io_workers must be greater than zero".into(),
            ));
        }
        if self.cursor_end_time.is_some_and(|value| value < 0) {
            return Err(Error::InvalidConfig(
                "cursor_end_time must not be negative".into(),
            ));
        }
        if self.auto_commit && self.auto_commit_interval.is_zero() {
            return Err(Error::InvalidConfig(
                "auto_commit_interval must be greater than zero".into(),
            ));
        }
        if self.shutdown_timeout.is_zero() {
            return Err(Error::InvalidConfig(
                "shutdown_timeout must be greater than zero".into(),
            ));
        }
        if self.processor_retry_limit == Some(0) {
            return Err(Error::InvalidConfig(
                "processor_retry_limit must be greater than zero when set".into(),
            ));
        }
        if self.processor_retry_interval.is_zero() {
            return Err(Error::InvalidConfig(
                "processor_retry_interval must be greater than zero".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_safe_and_compatible() {
        let config = ConsumerConfig::new("p", "l", "g", "c");
        assert_eq!(config.cursor_position, CursorPosition::End);
        assert_eq!(config.heartbeat_interval, Duration::from_secs(20));
        assert_eq!(config.heartbeat_timeout, Duration::from_secs(60));
        assert_eq!(config.max_fetch_log_group_count, 1000);
        assert_eq!(config.max_io_workers, 50);
        assert_eq!(config.processor_retry_limit, Some(10));
        assert_eq!(config.shutdown_timeout, Duration::from_secs(30));
        assert!(config.auto_commit);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_invalid_limits() {
        let config = ConsumerConfig::new("p", "l", "g", "c").max_fetch_log_group_count(1001);
        assert!(config.validate().is_err());
    }
}
