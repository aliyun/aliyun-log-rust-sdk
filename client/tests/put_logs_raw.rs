mod common;

#[cfg(test)]
mod tests {

    use crate::common::*;
    use aliyun_log_sdk::Client;
    use aliyun_log_sdk::*;
    use aliyun_log_sdk_protobuf::Log;
    use lazy_static::lazy_static;

    lazy_static! {
        static ref TEST_CLIENT: Client = {
            Client::from_config(
                Config::builder()
                    .access_key(&TEST_ENV.access_key_id, &TEST_ENV.access_key_secret)
                    .endpoint(&TEST_ENV.endpoint)
                    .build()
                    .unwrap(),
            )
            .unwrap()
        };
    }

    #[tokio::test]
    async fn test() {
        let mut log_group = aliyun_log_sdk_protobuf::LogGroup::new();
        let mut log = Log::new();
        log.set_time(chrono::Utc::now().timestamp().try_into().unwrap()); // unix timestamp
        log.add_content_kv("hello", "world");
        log_group.logs_mut().push(log);
        let bytes = log_group.encode().unwrap();
        let raw_size = bytes.len();
        let compressed = lz4::block::compress(bytes.as_ref(), None, false).unwrap();
        let project = &TEST_ENV.project;
        let logstore = &TEST_ENV.logstore;
        TEST_CLIENT
            .put_logs_raw(project, logstore)
            .data(compressed.into())
            .raw_size(raw_size)
            .compress_type("lz4".to_string())
            .send()
            .await
            .unwrap();
    }
}
