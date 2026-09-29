use std::{sync::Arc, time::Instant};

use aliyun_log_sdk_protobuf::{Log, LogGroup};

use crate::{state::Submission, SubmissionId};

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub(crate) struct Key {
    pub logstore: usize,
    pub source: String,
    pub topic: String,
}

pub(crate) struct Envelope {
    pub key: Key,
    pub submission: Submission,
    pub log: PreparedLog,
}

/// Single-log admission keeps both the log and its estimated size inline.
pub(crate) struct PreparedLog {
    pub log: Log,
    pub size: usize,
}

pub(crate) enum Command {
    Log(Envelope),
    Flush,
}

pub(crate) struct Batch {
    pub key: Key,
    pub group: LogGroup,
    /// Estimated log-entry bytes; group metadata uses reserved request headroom.
    pub size: usize,
    pub oldest: Instant,
    pub sealed_at: Option<Instant>,
    pub submission_ids: Vec<SubmissionId>,
    pub pack_id: Option<Arc<str>>,
}

impl Batch {
    pub fn new(key: Key, admitted: Instant) -> Self {
        let mut group = LogGroup::new();
        group.set_source(&key.source).set_topic(&key.topic);
        Self {
            key,
            group,
            size: 0,
            oldest: admitted,
            sealed_at: None,
            submission_ids: Vec::new(),
            pack_id: None,
        }
    }

    pub fn push_one(&mut self, log: Log, size: usize, submission: Submission) {
        self.group.add_log(log);
        self.size += size;
        self.oldest = self.oldest.min(submission.admitted);
        self.submission_ids.push(submission.id);
    }
}

/// Estimate one log without allocating an input Vec or a size Vec.
pub(crate) fn measure_log(log: &Log) -> (usize, usize) {
    // Reserve 6 bytes for uint32 time, 5 for optional fixed32 time_ns and
    // 5 for the enclosing log tag/length. No timestamp-dependent sizing.
    let mut raw_bytes = 16usize;
    for content in log.contents() {
        raw_bytes = raw_bytes
            .saturating_add(content.key().len())
            .saturating_add(content.value().len());
    }
    // Three tag/length pairs per content (content, key, value), estimated
    // at 2 bytes each. Long strings may need more length bytes; this is not
    // an upper bound.
    let size = raw_bytes.saturating_add(log.contents().len().saturating_mul(6));
    (size, raw_bytes)
}
