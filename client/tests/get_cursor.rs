mod common;

#[cfg(test)]
mod tests {
    use crate::common::*;
    use aliyun_log_sdk::Client;
    use aliyun_log_sdk::*;
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

    fn init() {
        let _ = env_logger::builder()
            .is_test(true)
            .filter_level(log::LevelFilter::Debug)
            .try_init();
    }

    #[tokio::test]
    async fn test() {
        init();
        let project = &TEST_ENV.project;
        let logstore = &TEST_ENV.logstore;

        let resp = TEST_CLIENT
            .list_shards(project, logstore)
            .send()
            .await
            .unwrap();
        let shards = resp.get_body().shards();
        let shard_id = shards[0].shard_id();

        let resp = TEST_CLIENT
            .get_cursor(project, logstore, *shard_id)
            .cursor_pos(get_cursor_models::CursorPos::Begin)
            .send()
            .await
            .unwrap();
        println!("{}", resp.get_body().cursor());
    }
}
