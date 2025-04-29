use std::borrow::Cow;

use crate::{
    error::{DecodeError, EncodeError},
    internal, Log, LogContent, LogGroup, LogGroupList, LogTag,
};
use quick_protobuf::{BytesReader, MessageRead, MessageWrite, Writer};

impl<'a> internal::LogGroupList<'a> {
    pub(crate) fn from_bytes(bytes: &'a [u8]) -> crate::error::Result<Self, crate::Error> {
        let mut reader = BytesReader::from_bytes(bytes);
        internal::LogGroupList::from_reader(&mut reader, bytes)
            .map_err(|e| crate::Error::Decode(DecodeError::Quick(e)))
    }
}

impl internal::LogGroup<'_> {
    pub(crate) fn to_bytes(&self) -> crate::error::Result<Vec<u8>, crate::Error> {
        let mut buf = Vec::new();
        let mut writer = Writer::new(&mut buf);
        self.write_message(&mut writer)
            .map_err(|e| crate::Error::Encode(EncodeError::Quick(e)))?;
        Ok(buf)
    }
}

// inner -> outter, copy to string
impl<'a> From<internal::LogGroupList<'a>> for LogGroupList {
    fn from(value: internal::LogGroupList<'a>) -> Self {
        Self {
            log_groups: value
                .log_groups
                .into_iter()
                .map(|log_group| LogGroup {
                    topic: log_group.topic.map(|s| s.to_string()),
                    source: log_group.source.map(|s| s.to_string()),
                    log_tags: log_group
                        .log_tags
                        .into_iter()
                        .map(|log_tag| LogTag {
                            key: log_tag.key.to_string(),
                            value: log_tag.value.to_string(),
                        })
                        .collect(),
                    logs: log_group
                        .logs
                        .into_iter()
                        .map(|log| Log {
                            time: log.time,
                            contents: log
                                .contents
                                .into_iter()
                                .map(|content| LogContent {
                                    key: content.key.to_string(),
                                    value: content.value.to_string(),
                                })
                                .collect(),
                            time_ns: log.time_ns,
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

// outter to inner, only ref
impl<'a> From<&'a LogGroup> for internal::LogGroup<'a> {
    fn from(log_group: &'a LogGroup) -> Self {
        let mut res = Self {
            topic: log_group.topic.clone().map(Cow::Owned),
            source: log_group.source.clone().map(Cow::Owned),
            ..Default::default()
        };

        res.logs.reserve(log_group.logs.len());
        for log in &log_group.logs {
            let contents = log
                .contents
                .iter()
                .map(|content| internal::LogContent {
                    key: Cow::Borrowed(&content.key),
                    value: Cow::Borrowed(&content.value),
                })
                .collect();
            res.logs.push(internal::Log {
                time: log.time,
                contents,
                time_ns: log.time_ns,
            });
        }
        res.log_tags = log_group
            .log_tags
            .iter()
            .map(|tag| internal::LogTag {
                key: Cow::Borrowed(&tag.key),
                value: Cow::Borrowed(&tag.value),
            })
            .collect();
        res
    }
}
