use aliyun_log_producer::log;
use std::{
    cell::RefCell,
    time::{Duration, UNIX_EPOCH},
};

#[test]
fn explicit_time_and_fields_are_evaluated_once_in_order() {
    let calls = RefCell::new(Vec::new());
    let entry = log!(time = {
        calls.borrow_mut().push("time");
        UNIX_EPOCH + Duration::new(1_700_000_000, 123_456_789)
    };
        { calls.borrow_mut().push("key"); String::from("tag") }: {
            calls.borrow_mut().push("value"); "a"
        },
        "tag": String::from("b"),
    );
    assert_eq!(*calls.borrow(), ["time", "key", "value"]);
    assert_eq!(*entry.time(), 1_700_000_000);
    assert_eq!(*entry.time_ns(), None);
    let pairs: Vec<_> = entry
        .contents()
        .iter()
        .map(|field| (field.key().as_str(), field.value().as_str()))
        .collect();
    assert_eq!(pairs, [("tag", "a"), ("tag", "b")]);
}

#[test]
fn automatic_and_empty_forms_work_outside_the_crate() {
    let before = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    let key = String::from("message");
    let entry = log!((key): format!("order {}", 123),);
    let after = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    let actual = *entry.time() as u64;
    assert!(before.as_secs() <= actual && actual <= after.as_secs());
    assert_eq!(*entry.time_ns(), None);
    assert_eq!(entry.contents()[0].value(), "order 123");
    assert!(log!().contents().is_empty());
    let empty = log!(time = UNIX_EPOCH;);
    assert_eq!(*empty.time(), 0);
    assert_eq!(*empty.time_ns(), None);
    assert!(empty.contents().is_empty());
}

#[test]
fn explicit_time_clamps_to_the_wire_timestamp_range() {
    let before_epoch = log!(time = UNIX_EPOCH - Duration::from_secs(1););
    let after_range = log!(time = UNIX_EPOCH + Duration::from_secs(u32::MAX as u64 + 1););
    assert_eq!(*before_epoch.time(), 0);
    assert_eq!(*after_range.time(), u32::MAX);
}
