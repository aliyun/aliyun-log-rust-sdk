use crate::error::Result;
use getset::{Getters, MutGetters, Setters};

#[doc(hidden)]
/// This struct is for internal use only.
pub type LogGroupImpl<'a> = crate::internal::LogGroup<'a>;

#[doc(hidden)]
/// This struct is for internal use only.
pub type LogGroupListImpl<'a> = crate::internal::LogGroupList<'a>;

impl LogGroupList {
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(LogGroupListImpl::from_bytes(bytes)?.into())
    }
}

impl LogGroup {
    pub fn encode(&self) -> Result<Vec<u8>> {
        LogGroupImpl::from(self).to_bytes()
    }
}

/// A list of log groups.
#[derive(Default, Clone, PartialEq, Debug, Getters, MutGetters, Setters)]
pub struct LogGroupList {
    #[getset(get = "pub", get_mut = "pub")]
    pub(crate) log_groups: Vec<LogGroup>,
}

/// A group of logs.
///
/// # Examples
///
/// Build a log group:
/// ```
/// use aliyun_log_sdk_protobuf::{Log, LogGroup};
/// let mut log_group = LogGroup::new();
/// log_group.set_topic("mytopic");
/// log_group.set_source("127.0.0.1");
///
/// let mut log = Log::new();
/// log.set_time(1690254376)
///     .add_content_kv("key1", "value1")
///     .add_content_kv("hello", "world");
/// log_group
///     .add_log(log)
///     .add_log_tag_kv("tagKey", "tagValue");
/// println!("{:?}", log_group);
/// ```
#[derive(Default, Clone, PartialEq, Debug, Getters, MutGetters, Setters)]
pub struct LogGroup {
    /// List of logs.
    #[getset(get = "pub", get_mut = "pub")]
    pub(crate) logs: Vec<Log>,

    /// Log topic, a user-defined field used to distinguish log data with different characteristics.
    #[getset(get = "pub")]
    pub(crate) topic: Option<String>,

    /// Source of the log. For example, the IP address of the machine that generated the log.
    #[getset(get = "pub")]
    pub(crate) source: Option<String>,

    /// List of log tags, where tags represent common characteristics of a set of logs.
    #[getset(get = "pub", get_mut = "pub")]
    pub(crate) log_tags: Vec<LogTag>,
}

impl From<LogGroupList> for Vec<LogGroup> {
    fn from(value: LogGroupList) -> Self {
        value.log_groups
    }
}

impl LogGroup {
    pub fn new() -> Self {
        LogGroup::default()
    }

    pub fn add_log(&mut self, log: Log) -> &mut Self {
        self.logs.push(log);
        self
    }

    pub fn add_log_tag(&mut self, log_tag: LogTag) -> &mut Self {
        self.log_tags.push(log_tag);
        self
    }

    pub fn add_log_tag_kv(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self {
        self.log_tags.push(LogTag {
            key: key.into(),
            value: value.into(),
        });
        self
    }

    pub fn set_source(&mut self, source: impl Into<String>) -> &mut Self {
        self.source = Some(source.into());
        self
    }

    pub fn set_topic(&mut self, topic: impl Into<String>) -> &mut Self {
        self.topic = Some(topic.into());
        self
    }
}

/// A log.
///
/// # Examples
///
/// Build a log:
///
/// ```
/// use aliyun_log_sdk_protobuf::Log;
/// let mut log = Log::from_unixtime(1690254376);
/// log.add_content_kv("key1", "value1")
///    .add_content_kv("hello", "world");
/// log.set_time_ns(123456789);
/// println!("{:?}", log);
/// ```
#[derive(Default, Clone, PartialEq, Debug, Getters, MutGetters, Setters)]
pub struct Log {
    /// The timestamp of the log in Unix format, e.g., 1690254376.
    #[getset(get = "pub", set = "pub")]
    pub(crate) time: u32,

    /// A list of log contents, where each entry consists of a key-value pair, both as strings.
    #[getset(get = "pub", get_mut = "pub")]
    pub(crate) contents: Vec<LogContent>,

    /// The nanosecond component of the log timestamp, ranging from 0 to 999,999,999.
    #[getset(get = "pub")]
    pub(crate) time_ns: Option<u32>,
}

impl Log {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_unixtime(time: u32) -> Self {
        Self {
            time,
            ..Self::default()
        }
    }
    pub fn add_content(&mut self, content: LogContent) -> &mut Self {
        self.contents.push(content);
        self
    }

    pub fn add_content_kv(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> &mut Self {
        self.contents.push(LogContent {
            key: key.into(),
            value: value.into(),
        });
        self
    }
    pub fn set_time_ns(&mut self, time_ns: u32) -> &mut Self {
        self.time_ns = Some(time_ns);
        self
    }
}

#[derive(Default, Clone, PartialEq, Debug, Getters, MutGetters, Setters)]
pub struct LogContent {
    /// Log key.
    #[getset(get = "pub", set = "pub")]
    pub(crate) key: String,

    /// Log value.
    #[getset(get = "pub", set = "pub")]
    pub(crate) value: String,
}

impl LogContent {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Default, Clone, PartialEq, Debug, Getters, MutGetters, Setters)]
pub struct LogTag {
    /// Tag key.
    #[getset(get = "pub", set = "pub")]
    pub(crate) key: String,

    /// Tag value.
    #[getset(get = "pub", set = "pub")]
    pub(crate) value: String,
}

impl LogTag {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_log_group_list_bytes(encoded_log_group: &[u8]) -> Vec<u8> {
        let mut buffer = Vec::new();
        prost::encoding::encode_key(1, prost::encoding::WireType::LengthDelimited, &mut buffer);

        prost::encoding::encode_length_delimiter(encoded_log_group.len(), &mut buffer)
            .expect("Cannot encode!");
        println!("buffer len: {}", buffer.len());
        println!("encoded_log_group len: {}", encoded_log_group.len());
        println!("buffer: {:?}", buffer);
        buffer.extend_from_slice(encoded_log_group);
        buffer
    }

    #[test]
    fn build_log() {
        let mut log = Log::from_unixtime(1690254376);
        log.add_content_kv("key1", "value1")
            .add_content_kv("hello", "world");
        log.set_time_ns(123456789);
        println!("{:?}", log)
    }

    #[test]
    fn build_log_group() {
        let mut log_group = LogGroup::new();
        log_group.set_topic("mytopic");
        log_group.set_source("127.0.0.1");
        let mut log = Log::from_unixtime(1690254376);
        log.add_content_kv("key", "value");
        log_group.add_log(log).add_log_tag_kv("tagKey", "tagValue");
        println!("{:?}", log_group);
    }

    #[allow(dead_code)]
    fn print_hex(data: &[u8]) {
        for byte in data {
            print!("{:02x} ", byte);
        }
        println!();
    }

    #[test]
    fn encode() {
        let mut log_group = LogGroup::new();
        log_group.set_topic("mytopic");
        log_group.set_source("127.0.0.1");
        for _ in 0..100 {
            let mut log = Log::from_unixtime(1690254376);
            log.add_content_kv("key", "value");
            log_group.add_log(log).add_log_tag_kv("tagKey", "tagValue");
        }
        log_group.encode().unwrap();
    }

    #[test]
    fn decode() {
        let mut log_group = LogGroup::new();
        log_group.set_topic("mytopic");
        log_group.set_source("127.0.0.1");
        for _ in 0..100 {
            let mut log = Log::from_unixtime(1690254376);
            log.add_content_kv("key", "value");
            log_group.add_log(log).add_log_tag_kv("tagKey", "tagValue");
        }
        let encoded = log_group.encode().unwrap();
        let log_group_bytes = get_log_group_list_bytes(&encoded);
        LogGroupList::decode(&log_group_bytes).unwrap();
    }
}
