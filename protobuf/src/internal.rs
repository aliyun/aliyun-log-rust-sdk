// Automatically generated rust module for 'logs.proto' file

#![allow(non_snake_case)]
#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(unused_imports)]
#![allow(unknown_lints)]
#![allow(clippy::all)]
#![cfg_attr(rustfmt, rustfmt_skip)]


use std::borrow::Cow;
use quick_protobuf::{MessageInfo, MessageRead, MessageWrite, BytesReader, Writer, WriterBackend, Result};
use quick_protobuf::sizeofs::*;
use super::*;

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LogContent<'a> {
    pub key: Cow<'a, str>,
    pub value: Cow<'a, str>,
}

impl<'a> MessageRead<'a> for LogContent<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.key = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.value = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for LogContent<'a> {
    fn get_size(&self) -> usize {
        0
        + 1 + sizeof_len((&self.key).len())
        + 1 + sizeof_len((&self.value).len())
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        w.write_with_tag(10, |w| w.write_string(&**&self.key))?;
        w.write_with_tag(18, |w| w.write_string(&**&self.value))?;
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Log<'a> {
    pub time: u32,
    pub contents: Vec<internal::LogContent<'a>>,
    pub time_ns: Option<u32>,
}

impl<'a> MessageRead<'a> for Log<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(8) => msg.time = r.read_uint32(bytes)?,
                Ok(18) => msg.contents.push(r.read_message::<internal::LogContent>(bytes)?),
                Ok(37) => msg.time_ns = Some(r.read_fixed32(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for Log<'a> {
    fn get_size(&self) -> usize {
        0
        + 1 + sizeof_varint(*(&self.time) as u64)
        + self.contents.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
        + self.time_ns.as_ref().map_or(0, |_| 1 + 4)
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        w.write_with_tag(8, |w| w.write_uint32(*&self.time))?;
        for s in &self.contents { w.write_with_tag(18, |w| w.write_message(s))?; }
        if let Some(ref s) = self.time_ns { w.write_with_tag(37, |w| w.write_fixed32(*s))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LogTag<'a> {
    pub key: Cow<'a, str>,
    pub value: Cow<'a, str>,
}

impl<'a> MessageRead<'a> for LogTag<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.key = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(18) => msg.value = r.read_string(bytes).map(Cow::Borrowed)?,
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for LogTag<'a> {
    fn get_size(&self) -> usize {
        0
        + 1 + sizeof_len((&self.key).len())
        + 1 + sizeof_len((&self.value).len())
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        w.write_with_tag(10, |w| w.write_string(&**&self.key))?;
        w.write_with_tag(18, |w| w.write_string(&**&self.value))?;
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LogGroup<'a> {
    pub logs: Vec<internal::Log<'a>>,
    pub topic: Option<Cow<'a, str>>,
    pub source: Option<Cow<'a, str>>,
    pub log_tags: Vec<internal::LogTag<'a>>,
}

impl<'a> MessageRead<'a> for LogGroup<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.logs.push(r.read_message::<internal::Log>(bytes)?),
                Ok(26) => msg.topic = Some(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(34) => msg.source = Some(r.read_string(bytes).map(Cow::Borrowed)?),
                Ok(50) => msg.log_tags.push(r.read_message::<internal::LogTag>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for LogGroup<'a> {
    fn get_size(&self) -> usize {
        0
        + self.logs.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
        + self.topic.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + self.source.as_ref().map_or(0, |m| 1 + sizeof_len((m).len()))
        + self.log_tags.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.logs { w.write_with_tag(10, |w| w.write_message(s))?; }
        if let Some(ref s) = self.topic { w.write_with_tag(26, |w| w.write_string(&**s))?; }
        if let Some(ref s) = self.source { w.write_with_tag(34, |w| w.write_string(&**s))?; }
        for s in &self.log_tags { w.write_with_tag(50, |w| w.write_message(s))?; }
        Ok(())
    }
}

#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LogGroupList<'a> {
    pub(crate) log_groups: Vec<internal::LogGroup<'a>>,
}

impl<'a> MessageRead<'a> for LogGroupList<'a> {
    fn from_reader(r: &mut BytesReader, bytes: &'a [u8]) -> Result<Self> {
        let mut msg = Self::default();
        while !r.is_eof() {
            match r.next_tag(bytes) {
                Ok(10) => msg.log_groups.push(r.read_message::<internal::LogGroup>(bytes)?),
                Ok(t) => { r.read_unknown(bytes, t)?; }
                Err(e) => return Err(e),
            }
        }
        Ok(msg)
    }
}

impl<'a> MessageWrite for LogGroupList<'a> {
    fn get_size(&self) -> usize {
        0
        + self.log_groups.iter().map(|s| 1 + sizeof_len((s).get_size())).sum::<usize>()
    }

    fn write_message<W: WriterBackend>(&self, w: &mut Writer<W>) -> Result<()> {
        for s in &self.log_groups { w.write_with_tag(10, |w| w.write_message(s))?; }
        Ok(())
    }
}

