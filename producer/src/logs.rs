/// Build a log with inline string pairs, using the current time by default.
/// Use `time = expression;` to supply a [`std::time::SystemTime`].
/// Timestamps keep whole seconds, clamp to the SLS timestamp range, and
/// become zero before the Unix epoch.
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
        let time: ::std::time::SystemTime = $time;
        let elapsed = time.duration_since(::std::time::UNIX_EPOCH).unwrap_or_default();
        #[allow(unused_mut)]
        let mut entry = $crate::Log::from_unixtime(elapsed.as_secs().min(u32::MAX as u64) as u32);
        $(entry.add_content_kv($key, $value);)*
        entry
    }};
    ($($key:tt : $value:expr),* $(,)?) => {
        $crate::log!(time = ::std::time::SystemTime::now(); $($key : $value),*)
    };
}
