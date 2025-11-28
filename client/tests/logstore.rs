mod common;

#[cfg(test)]
mod tests {
    use crate::common::*;
    use aliyun_log_rust_sdk::Client;
    use aliyun_log_rust_sdk::FromConfig;
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

    /// Get test logstore name with suffix
    fn get_test_logstore_name() -> String {
        format!("{}-for-test", TEST_ENV.logstore)
    }

    /// Macro to clean up logstore at the start of test
    macro_rules! cleanup_logstore {
        ($client:expr, $project:expr, $logstore:expr) => {
            match $client.delete_logstore($project, $logstore).send().await {
                Ok(_) => {
                    // Wait a moment for logstore deletion to complete
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                Err(e) => {
                    // Only ignore if logstore doesn't exist
                    if !matches!(&e, aliyun_log_rust_sdk::Error::Server { error_code, .. } if error_code == "LogStoreNotExist")
                    {
                        eprintln!("Warning: Failed to cleanup logstore: {}", e);
                    }
                }
            }
        };
    }

    /// Macro to create logstore and handle AlreadyExist error
    macro_rules! create_logstore {
        ($client:expr, $project:expr, $logstore:expr, $shard_count:expr, $ttl:expr) => {
            match $client
                .create_logstore($project, $logstore)
                .shard_count($shard_count)
                .ttl($ttl)
                .send()
                .await
            {
                Ok(_) => {
                    // Wait a moment for logstore creation to complete
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                Err(e) => {
                    // Only accept LogStoreAlreadyExist error
                    if !matches!(&e, aliyun_log_rust_sdk::Error::Server { error_code, .. } if error_code == "LogStoreAlreadyExist")
                    {
                        panic!(
                            "Failed to create logstore, expected success or LogStoreAlreadyExist, got: {}",
                            e
                        );
                    }
                }
            }
        };
    }

    #[tokio::test]
    async fn test_logstore_lifecycle() {
        init();
        let project = &TEST_ENV.project;
        let logstore_name = get_test_logstore_name();

        // Clean up any existing logstore from previous test runs
        cleanup_logstore!(&TEST_CLIENT, project, &logstore_name);

        // Test 1: Create logstore
        create_logstore!(&TEST_CLIENT, project, &logstore_name, 2, 30);
        println!("✓ Created logstore: {}", logstore_name);

        // Test 2: Get logstore
        let get_resp = TEST_CLIENT
            .get_logstore(project, &logstore_name)
            .send()
            .await
            .unwrap();

        let logstore_info = get_resp.get_body();
        assert_eq!(logstore_info.logstore_name(), &logstore_name);
        assert_eq!(*logstore_info.shard_count(), 2);
        assert_eq!(*logstore_info.ttl(), 30);
        println!("✓ Got logstore: {}", logstore_name);

        // Test 3: List logstores
        let list_resp = TEST_CLIENT
            .list_logstores(project, 0, 100)
            .logstore_name(&logstore_name)
            .send()
            .await
            .unwrap();

        let logstores = list_resp.get_body().logstores();
        let found_logstore = logstores.iter().any(|name| name == &logstore_name);

        assert!(found_logstore, "Created logstore should be in the list");
        println!("✓ Listed logstores, found: {}", logstore_name);

        // Test 4: Update logstore
        TEST_CLIENT
            .update_logstore(project, &logstore_name)
            .ttl(60)
            .hot_ttl(7)
            .send()
            .await
            .unwrap();
        println!("✓ Updated logstore: {}", logstore_name);

        // Verify update
        let get_resp = TEST_CLIENT
            .get_logstore(project, &logstore_name)
            .send()
            .await
            .unwrap();

        let logstore_info = get_resp.get_body();
        assert_eq!(*logstore_info.ttl(), 60);
        if let Some(hot_ttl) = logstore_info.hot_ttl() {
            assert_eq!(*hot_ttl, 7);
        }
        println!("✓ Verified logstore update");

        // Test 5: Delete logstore
        TEST_CLIENT
            .delete_logstore(project, &logstore_name)
            .send()
            .await
            .unwrap();
        println!("✓ Deleted logstore: {}", logstore_name);

        // Wait a moment for deletion to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Verify deletion
        let get_result = TEST_CLIENT
            .get_logstore(project, &logstore_name)
            .send()
            .await;

        assert!(
            get_result.is_err(),
            "Logstore should not exist after deletion"
        );
        println!("✓ Verified logstore deletion");
    }
}
