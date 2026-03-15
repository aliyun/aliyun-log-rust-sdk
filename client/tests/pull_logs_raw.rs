mod common;

#[cfg(test)]
mod tests {
    use crate::common::*;
    use aliyun_log_rust_sdk::get_cursor_models::*;
    use aliyun_log_rust_sdk::Client;
    use aliyun_log_rust_sdk::*;
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
    async fn test_pull_logs_raw() {
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
            .cursor_pos(CursorPos::Begin)
            .send()
            .await
            .unwrap();
        let cursor = resp.get_body().cursor();

        let resp = TEST_CLIENT
            .pull_logs_raw(project, logstore, *shard_id)
            .cursor(cursor)
            .count(1000)
            .send()
            .await
            .unwrap();

        let body = resp.get_body();
        println!("Raw data size: {} bytes", body.data().len());
        println!("Log group count: {}", body.log_group_count());
        println!("Next cursor: {}", body.next_cursor());

        assert!(!body.next_cursor().is_empty());
    }

    #[tokio::test]
    async fn test_pull_logs_raw_with_query() {
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
            .cursor_pos(CursorPos::Begin)
            .send()
            .await
            .unwrap();
        let cursor = resp.get_body().cursor();

        let resp = TEST_CLIENT
            .pull_logs_raw(project, logstore, *shard_id)
            .cursor(cursor)
            .count(1000)
            .query("* | where 1 = 1")
            .send()
            .await
            .unwrap();

        let body = resp.get_body();
        println!("Raw data size: {} bytes", body.data().len());
        println!("Log group count: {}", body.log_group_count());

        assert!(!body.next_cursor().is_empty());
    }

    #[tokio::test]
    async fn test_pull_logs_raw_with_end_cursor() {
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
            .cursor_pos(CursorPos::Begin)
            .send()
            .await
            .unwrap();
        let start_cursor = resp.get_body().cursor();

        let resp = TEST_CLIENT
            .get_cursor(project, logstore, *shard_id)
            .cursor_pos(CursorPos::End)
            .send()
            .await
            .unwrap();
        let end_cursor = resp.get_body().cursor();

        let resp = TEST_CLIENT
            .pull_logs_raw(project, logstore, *shard_id)
            .cursor(start_cursor)
            .end_cursor(end_cursor)
            .count(1000)
            .send()
            .await
            .unwrap();

        let body = resp.get_body();
        println!("Raw data size: {} bytes", body.data().len());
        println!("Log group count: {}", body.log_group_count());
        println!("Next cursor: {}", body.next_cursor());
    }
}
