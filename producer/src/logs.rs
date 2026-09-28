use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::Log;

pub trait IntoLog {
    fn into_log(self) -> Log;
}

impl IntoLog for Log {
    fn into_log(self) -> Log {
        self
    }
}

impl IntoLog for HashMap<String, String> {
    fn into_log(self) -> Log {
        log(self)
    }
}

/// Build a log from string pairs, using the current time in whole seconds.
///
/// ```
/// use aliyun_log_producer::log;
/// let entry = log([("level", "INFO"), ("message", "hello")]);
/// assert_eq!(entry.contents().len(), 2);
/// ```
pub fn log<K, V>(contents: impl IntoIterator<Item = (K, V)>) -> Log
where
    K: Into<String>,
    V: Into<String>,
{
    log_at(SystemTime::now(), contents)
}

/// Build a log at the given system time, keeping only whole seconds.
/// Seconds are clamped to the SLS timestamp range; times before the Unix epoch become zero.
/// String pairs retain their order and may contain duplicate keys.
///
/// ```
/// use aliyun_log_producer::log_at;
/// use std::time::{Duration, SystemTime};
/// let event_time = SystemTime::now() - Duration::from_secs(60);
/// let entry = log_at(event_time, [("message", "imported log")]);
/// assert_eq!(entry.contents().len(), 1);
/// ```
pub fn log_at<K, V>(time: SystemTime, contents: impl IntoIterator<Item = (K, V)>) -> Log
where
    K: Into<String>,
    V: Into<String>,
{
    let elapsed = time.duration_since(UNIX_EPOCH).unwrap_or_default();
    let mut log = Log::from_unixtime(elapsed.as_secs().min(u32::MAX as u64) as u32);
    for (key, value) in contents {
        log.add_content_kv(key, value);
    }
    log
}

/// Construct an empty log using the current time in whole seconds.
pub fn log_now() -> Log {
    log(std::iter::empty::<(&str, &str)>())
}

/// Build a log with inline string pairs, using the current time by default.
/// Use `time = expression;` to supply a [`std::time::SystemTime`].
/// Keys and values accept string literals or owned strings; duplicate keys are kept.
/// Wrap key expressions in parentheses, e.g. `log!((make_key()): "value")`.
///
/// ```
/// use aliyun_log_producer::log;
/// let entry = log!("level": "INFO", "message": format!("order {}", 123));
/// assert_eq!(entry.contents().len(), 2);
/// ```
///
/// ```
/// use aliyun_log_producer::log;
/// use std::time::{Duration, SystemTime};
/// let event_time = SystemTime::now() - Duration::from_secs(60);
/// let entry = log!(time = event_time; "message": "imported log");
/// assert_eq!(entry.contents().len(), 1);
/// ```
#[macro_export]
macro_rules! log {
    (time = $time:expr; $($key:tt : $value:expr),* $(,)?) => {{
        #[allow(unused_mut)]
        let mut entry = $crate::log_at($time, ::std::iter::empty::<(&str, &str)>());
        $(entry.add_content_kv($key, $value);)*
        entry
    }};
    ($($key:tt : $value:expr),* $(,)?) => {
        $crate::log!(time = ::std::time::SystemTime::now(); $($key : $value),*)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_time_keeps_seconds_and_ordered_owned_contents() {
        let time = UNIX_EPOCH + std::time::Duration::new(1_700_000_000, 123_456_789);
        let entry = log_at(
            time,
            vec![
                ("tag".to_owned(), "a".to_owned()),
                ("tag".into(), "b".into()),
            ],
        );
        assert_eq!(*entry.time(), 1_700_000_000);
        assert_eq!(*entry.time_ns(), None);
        let pairs: Vec<_> = entry
            .contents()
            .iter()
            .map(|content| (content.key().as_str(), content.value().as_str()))
            .collect();
        assert_eq!(pairs, [("tag", "a"), ("tag", "b")]);
    }

    #[test]
    fn automatic_time_is_current_in_seconds() {
        let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let entry = log([("message", "hello")]);
        let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let actual = *entry.time() as u64;
        assert!(before.as_secs() <= actual && actual <= after.as_secs());
        assert_eq!(*entry.time_ns(), None);
        assert_eq!(entry.contents()[0].value(), "hello");
        assert!(log_now().contents().is_empty());
    }
}
