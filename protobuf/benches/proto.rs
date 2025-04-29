use aliyun_log_sdk_protobuf::{Log, LogGroup, LogGroupList};
use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
fn prepare_log_group(log_count: usize) -> LogGroup {
    let mut log_group = LogGroup::new();
    for _ in 0..log_count {
        let mut log = Log::new();
        log.set_time(1694253376);
        log.add_content_kv("Owner".to_string(), "1654218965343050".to_string());
        log.add_content_kv(
            "ProjectName".to_string(),
            ":cloudlens-test-cn-beijing-stg".to_string(),
        );
        log.add_content_kv("GroupCount".to_string(), "778".to_string());
        log.add_content_kv("NetFlow".to_string(), "0".to_string());
        log.add_content_kv("CallerType".to_string(), "Sts".to_string());
        log.add_content_kv("OutFlow".to_string(), "31145".to_string());
        log.add_content_kv(
            "Cursor".to_string(),
            "MTc0NDM0MDIzOTQzNTM0NzM2Mg==".to_string(),
        );
        log.add_content_kv("Source".to_string(), "100.68.147.135".to_string());
        log.add_content_kv("InFlow".to_string(), "0".to_string());
        log.add_content_kv("RoleSessionName".to_string(), "sls_dispatch".to_string());
        log.add_content_kv("APIVersion".to_string(), "0.6.0".to_string());
        log.add_content_kv("NetworkType".to_string(), "intranet".to_string());
        log.add_content_kv("UserAgent".to_string(), "sls-cpp-sdk v0.6.1".to_string());
        log.add_content_kv("Status".to_string(), "200".to_string());
        log.add_content_kv(
            "RequestId".to_string(),
            "67F88F0A0CA59FE9DD227CAE".to_string(),
        );
        log.add_content_kv("LogStore".to_string(), "oss-access-log".to_string());
        log.add_content_kv("ProjectId".to_string(), "38760".to_string());
        log.add_content_kv("__THREAD__".to_string(), "57304".to_string());
        log.add_content_kv("Method".to_string(), "PullData".to_string());
        log.add_content_kv("Acl".to_string(), "0".to_string());
        log.add_content_kv("ClientIP".to_string(), "100.68.147.135".to_string());
        log.add_content_kv("RoleId".to_string(), "310438458647495662".to_string());
        log.add_content_kv("CompressType".to_string(), "zstd".to_string());
        log.add_content_kv("Latency".to_string(), "6486".to_string());
        log.add_content_kv("Role".to_string(), "aliyunlogdispatchrole".to_string());
        log.add_content_kv("RawOutflow".to_string(), "1553725".to_string());
        log.add_content_kv(
            "NextCursor".to_string(),
            "MTc0NDM0MDIzOTQzNTM0ODE0MA==".to_string(),
        );
        log.add_content_kv("UserId".to_string(), "248".to_string());
        log.add_content_kv(
            "AccessKeyId".to_string(),
            "STS.NWMLZ5XvntWPyLd8rjkXzQgvA".to_string(),
        );
        log.add_content_kv("Shard".to_string(), "115".to_string());
        log.add_content_kv("Vip".to_string(), "100.67.27.217".to_string());
        log.add_content_kv("AliUid".to_string(), "1418436495972562".to_string());
        log.add_content_kv("ExOutFlow".to_string(), "0".to_string());
        log.add_content_kv("RequestType".to_string(), "read".to_string());
        log.add_content_kv("microtime".to_string(), "1744342794704865".to_string());
        log_group.add_log(log);
    }
    log_group.add_log_tag_kv("__hostname__", "hellow");
    log_group.add_log_tag_kv("__source__", "127.0.0.1");
    log_group
}

fn encode(log_group: &LogGroup) -> Vec<u8> {
    #[allow(unused_must_use)]
    log_group.encode().expect("Cannot encode!")
}
fn decode(encoded_log_group: &[u8]) {
    LogGroupList::decode(encoded_log_group).expect("Cannot decode!");
}

fn get_log_group_list_bytes(encoded_log_group: &[u8]) -> Vec<u8> {
    let mut buffer = Vec::new();
    prost::encoding::encode_key(1, prost::encoding::WireType::LengthDelimited, &mut buffer);

    prost::encoding::encode_length_delimiter(encoded_log_group.len(), &mut buffer)
        .expect("Cannot encode!");
    buffer.extend_from_slice(encoded_log_group);
    buffer
}

fn criterion_benchmark(c: &mut Criterion) {
    let log_group = prepare_log_group(100);
    c.bench_function("encode", |b| b.iter(|| encode(black_box(&log_group))));

    let encoded = encode(&log_group);
    println!("{}", encoded.len());

    let log_group_bytes = get_log_group_list_bytes(&encoded);
    c.bench_function("decode", |b| b.iter(|| decode(black_box(&log_group_bytes))));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
