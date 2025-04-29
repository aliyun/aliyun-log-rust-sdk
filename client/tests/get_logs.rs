mod common;

#[cfg(test)]
mod tests {
    use crate::common::*;
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

    #[tokio::test]
    async fn test() {
        let now: i64 = chrono::Utc::now().timestamp();

        let project = &TEST_ENV.project;
        let logstore = &TEST_ENV.logstore;
        let resp = TEST_CLIENT
            .get_logs(project, logstore)
            .from(now - 3000)
            .to(now)
            .offset(0)
            .lines(100)
            .query("*")
            .send()
            .await
            .unwrap();
        assert!(resp.get_body().is_complete());
        println!("{:?}", resp.get_body());
    }
}
