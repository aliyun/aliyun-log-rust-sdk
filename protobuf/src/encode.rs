//! Encode the facade directly, avoiding per-log intermediate message allocations.
//! Keep field numbers and presence semantics in sync with logs.proto; tests compare
//! the bytes with the generated encoder, which remains the wire-format reference.
use crate::{Log, LogContent, LogGroup, LogTag};
use quick_protobuf::{
    sizeofs::{sizeof_len, sizeof_varint},
    MessageWrite, Writer, WriterBackend,
};

impl MessageWrite for LogContent {
    fn get_size(&self) -> usize {
        1 + sizeof_len(self.key.len()) + 1 + sizeof_len(self.value.len())
    }
    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> quick_protobuf::Result<()> {
        w.write_with_tag(10, |w| w.write_string(&self.key))?;
        w.write_with_tag(18, |w| w.write_string(&self.value))
    }
}
impl MessageWrite for LogTag {
    fn get_size(&self) -> usize {
        1 + sizeof_len(self.key.len()) + 1 + sizeof_len(self.value.len())
    }
    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> quick_protobuf::Result<()> {
        w.write_with_tag(10, |w| w.write_string(&self.key))?;
        w.write_with_tag(18, |w| w.write_string(&self.value))
    }
}
impl MessageWrite for Log {
    fn get_size(&self) -> usize {
        1 + sizeof_varint(self.time as u64)
            + self
                .contents
                .iter()
                .map(|c| 1 + sizeof_len(c.get_size()))
                .sum::<usize>()
            + self.time_ns.map_or(0, |_| 5)
    }
    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> quick_protobuf::Result<()> {
        w.write_with_tag(8, |w| w.write_uint32(self.time))?;
        for content in &self.contents {
            w.write_with_tag(18, |w| w.write_message(content))?;
        }
        if let Some(ns) = self.time_ns {
            w.write_with_tag(37, |w| w.write_fixed32(ns))?;
        }
        Ok(())
    }
}
impl MessageWrite for LogGroup {
    fn get_size(&self) -> usize {
        self.logs
            .iter()
            .map(|log| 1 + sizeof_len(log.get_size()))
            .sum::<usize>()
            + self.topic.as_ref().map_or(0, |s| 1 + sizeof_len(s.len()))
            + self.source.as_ref().map_or(0, |s| 1 + sizeof_len(s.len()))
            + self
                .log_tags
                .iter()
                .map(|tag| 1 + sizeof_len(tag.get_size()))
                .sum::<usize>()
    }
    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> quick_protobuf::Result<()> {
        for log in &self.logs {
            w.write_with_tag(10, |w| w.write_message(log))?;
        }
        if let Some(topic) = &self.topic {
            w.write_with_tag(26, |w| w.write_string(topic))?;
        }
        if let Some(source) = &self.source {
            w.write_with_tag(34, |w| w.write_string(source))?;
        }
        for tag in &self.log_tags {
            w.write_with_tag(50, |w| w.write_message(tag))?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal;

    fn generated_encoding(group: &LogGroup) -> Vec<u8> {
        let mut output = Vec::new();
        internal::LogGroup::from(group)
            .write_message(&mut Writer::new(&mut output))
            .unwrap();
        output
    }

    #[test]
    fn optional_fields_preserve_presence_and_fixed32_boundaries() {
        for topic in [None, Some(""), Some("主题")] {
            for source in [None, Some(""), Some("host")] {
                for ns in [None, Some(0), Some(999_999_999), Some(u32::MAX)] {
                    let mut group = LogGroup::new();
                    if let Some(topic) = topic {
                        group.set_topic(topic);
                    }
                    if let Some(source) = source {
                        group.set_source(source);
                    }
                    let mut log = Log::new();
                    if let Some(ns) = ns {
                        log.set_time_ns(ns);
                    }
                    group
                        .add_log(log)
                        .add_log_tag_kv("", "")
                        .add_log_tag_kv("tag", "值");
                    assert_eq!(group.encode().unwrap(), generated_encoding(&group));
                    assert_eq!(group.encode().unwrap().len(), group.get_size());
                }
            }
        }
    }

    #[test]
    fn reused_buffer_replaces_previous_output() {
        let mut output = Vec::new();
        for size in [1024 * 1024, 0, 1, 128, 16384, 0] {
            let mut group = LogGroup::new();
            let mut log = Log::from_unixtime(u32::MAX);
            log.set_time_ns(999_999_999)
                .add_content_kv("字段", "x".repeat(size));
            group.set_source("").set_topic("主题").add_log(log);
            group.encode_into(&mut output).unwrap();
            assert_eq!(output, generated_encoding(&group));
        }
        let capacity = output.capacity();
        LogGroup::new().encode_into(&mut output).unwrap();
        assert!(output.is_empty());
        assert_eq!(output.capacity(), capacity);
    }

    #[test]
    fn direct_encoding_matches_generated_messages_at_wire_boundaries() {
        for len in [0, 1, 127, 128, 16383, 16384, 1024 * 1024] {
            for time in [0, 127, 128, 1_700_000_000, u32::MAX] {
                let mut group = LogGroup::new();
                let mut log = Log::from_unixtime(time);
                log.add_content_kv("字段", "x".repeat(len))
                    .add_content_kv("", "");
                group.add_log(log.clone());
                log.set_time_ns(999_999_999);
                group.add_log(log);
                for metadata in [false, true] {
                    if metadata {
                        group
                            .set_source("")
                            .set_topic("主题")
                            .add_log_tag_kv("__pack_id__", "0123-4567");
                    }
                    let generated = generated_encoding(&group);
                    let direct = group.encode().unwrap();
                    assert_eq!(direct, generated);
                    assert_eq!(direct.len(), group.get_size());
                }
            }
        }
        assert_eq!(LogGroup::new().encode().unwrap(), Vec::<u8>::new());
    }
}
