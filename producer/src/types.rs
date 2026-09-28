use crate::DeliveryError;

/// Internal admission sequence used for pending submissions and flush watermarks.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct SubmissionId(pub u64);

/// The submitted log is confirmed delivered, or its delivery ended in error.
/// An error does not prove that the service received nothing; retries can duplicate logs.
///
/// # Errors
///
/// Callbacks receive the following delivery errors:
///
/// - [`DeliveryError::Server`]: a terminal service error or exhausted retries.
/// - [`DeliveryError::Network`]: an HTTP transport error after allowed retries.
/// - [`DeliveryError::Credentials`]: credentials could not be obtained.
/// - [`DeliveryError::Timeout`]: the overall delivery deadline expired.
/// - [`DeliveryError::InvalidResponse`]: the client could not parse the response.
/// - [`DeliveryError::Internal`]: local processing, a caught panic, a pipeline
///   failure, or a terminal client error without a more specific category.
///
/// Callback results describe the final outcome after retry handling. Admission errors
/// are returned directly by [`crate::LogstoreWriter::send_with_callback`] and never invoke the callback.
pub type DeliveryResult = Result<(), DeliveryError>;

/// One final [`DeliveryResult`] per accepted submission, executed by the event consumer.
/// See [`crate::LogstoreWriter::send_with_callback`] for execution and failure behavior.
pub(crate) type Callback = Box<dyn FnOnce(DeliveryResult) + Send + 'static>;

/// Cloneable source and topic options for one log submission.
/// Source/topic default to empty, with no distinction between unset and empty.
/// Pass a callback separately to [`crate::LogstoreWriter::send_with_callback`].
/// Construct with `SendOptions::default()` and the `with_*` methods.
///
/// ```
/// use aliyun_log_producer::SendOptions;
/// let options = SendOptions::default().with_source("host").with_topic("app");
/// assert_eq!(options.source(), "host");
/// ```
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct SendOptions {
    /// Aggregation source. Defaults to empty; unset and empty have identical semantics.
    pub(crate) source: String,
    /// Aggregation topic. Defaults to empty; unset and empty have identical semantics.
    pub(crate) topic: String,
}

impl SendOptions {
    /// Set the aggregation source; an empty value has the same meaning as omission.
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }

    /// Read the configured value; see [`Self::with_source`].
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Set the aggregation topic; an empty value has the same meaning as omission.
    pub fn with_topic(mut self, topic: impl Into<String>) -> Self {
        self.topic = topic.into();
        self
    }

    /// Read the configured value; see [`Self::with_topic`].
    pub fn topic(&self) -> &str {
        &self.topic
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub(crate) enum ProducerState {
    Running,
    Closing,
    Closed,
}
