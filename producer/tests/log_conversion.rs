use aliyun_log_producer::IntoLog;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn owned_hashmap_converts_to_log_with_current_time() {
    let fields = HashMap::from([
        (String::from("level"), String::from("INFO")),
        (String::from("message"), String::from("你好 SLS")),
    ]);
    let before = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    let log = fields.into_log();
    let after = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    let actual: HashMap<_, _> = log
        .contents()
        .iter()
        .map(|field| (field.key().as_str(), field.value().as_str()))
        .collect();
    assert_eq!(
        actual,
        HashMap::from([("level", "INFO"), ("message", "你好 SLS")])
    );
    let timestamp = *log.time() as u64;
    assert!(before.as_secs() <= timestamp && timestamp <= after.as_secs());
    assert_eq!(*log.time_ns(), None);
}
