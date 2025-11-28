mod common;

#[cfg(test)]
mod tests {
    use crate::common::*;
    use aliyun_log_rust_sdk::Client;
    use aliyun_log_rust_sdk::FromConfig;
    use aliyun_log_rust_sdk::*;
    use lazy_static::lazy_static;
    use std::collections::HashMap;

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

    /// Get test logstore name for index testing
    fn get_test_index_logstore_name() -> String {
        format!("{}-index-for-test", TEST_ENV.logstore)
    }

    /// Macro to clean up logstore at the start of test
    macro_rules! cleanup_logstore {
        ($client:expr, $project:expr, $logstore:expr) => {
            match $client.delete_logstore($project, $logstore).send().await {
                Ok(_) => {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                Err(e) => {
                    if !matches!(&e, aliyun_log_rust_sdk::Error::Server { error_code, .. } if error_code == "LogStoreNotExist")
                    {
                        eprintln!("Warning: Failed to cleanup logstore: {}", e);
                    }
                }
            }
        };
    }

    /// Macro to create a temporary logstore for index testing
    macro_rules! create_logstore {
        ($client:expr, $project:expr, $logstore:expr) => {
            $client
                .create_logstore($project, $logstore)
                .shard_count(2)
                .ttl(1)
                .send()
                .await
                .expect("Failed to create logstore");

            // Wait for logstore to be ready
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        };
    }

    #[tokio::test]
    async fn test_index_crud() {
        init();

        let client = &TEST_CLIENT;
        let project = &TEST_ENV.project;
        let logstore_name = get_test_index_logstore_name();

        // Clean up any existing logstore and create a new one
        cleanup_logstore!(client, project, &logstore_name);
        create_logstore!(client, project, &logstore_name);

        // Step 1: Create index with full-text index and field indexes
        println!("Creating index with full-text and field indexes...");

        let full_text_index = FullTextIndex {
            case_sensitive: false,
            chn: true,
            token: token_list![",", " ", ";", "\n", "\t"],
        };

        let mut field_indexes = HashMap::new();

        // Text field index for "level"
        field_indexes.insert(
            "level".to_string(),
            FieldIndex::Text(IndexKeyText {
                case_sensitive: false,
                alias: None,
                chn: false,
                token: token_list![],
                doc_value: true,
            }),
        );

        // Long field index for "status_code"
        field_indexes.insert(
            "status_code".to_string(),
            FieldIndex::Long(IndexKeyLong {
                alias: None,
                doc_value: true,
            }),
        );

        // Double field index for "response_time"
        field_indexes.insert(
            "response_time".to_string(),
            FieldIndex::Double(IndexKeyDouble {
                alias: None,
                doc_value: true,
            }),
        );

        let index = Index::builder()
            .line(full_text_index)
            .keys(field_indexes)
            .build();

        let create_result = client
            .create_index(project, &logstore_name, index)
            .send()
            .await;

        assert!(
            create_result.is_ok(),
            "Failed to create index: {:?}",
            create_result.err()
        );

        // Wait for index to be created
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Step 2: Get index and verify
        println!("Getting index and verifying...");

        let get_result = client.get_index(project, &logstore_name).send().await;

        assert!(
            get_result.is_ok(),
            "Failed to get index: {:?}",
            get_result.err()
        );

        let response = get_result.unwrap();
        let retrieved_index = response.get_body();

        // Verify full-text index
        assert!(
            retrieved_index.line.is_some(),
            "Full-text index should exist"
        );
        let line = retrieved_index.line.as_ref().unwrap();
        assert_eq!(line.case_sensitive, false);
        assert_eq!(line.chn, true);

        // Verify field indexes
        assert!(retrieved_index.keys.is_some(), "Field indexes should exist");
        let keys = retrieved_index.keys.as_ref().unwrap();
        assert!(
            keys.contains_key("level"),
            "Should have 'level' field index"
        );
        assert!(
            keys.contains_key("status_code"),
            "Should have 'status_code' field index"
        );
        assert!(
            keys.contains_key("response_time"),
            "Should have 'response_time' field index"
        );

        println!("Index verified successfully!");

        // Step 3: Update index - add a new field index
        println!("Updating index...");

        let mut updated_field_indexes = HashMap::new();

        // Add original field indexes
        updated_field_indexes.insert(
            "level".to_string(),
            FieldIndex::Text(IndexKeyText {
                case_sensitive: false,
                alias: None,
                chn: false,
                token: token_list![],
                doc_value: true,
            }),
        );

        updated_field_indexes.insert(
            "status_code".to_string(),
            FieldIndex::Long(IndexKeyLong {
                alias: None,
                doc_value: true,
            }),
        );

        updated_field_indexes.insert(
            "response_time".to_string(),
            FieldIndex::Double(IndexKeyDouble {
                alias: None,
                doc_value: true,
            }),
        );

        // Add a new text field index for "user_id"
        updated_field_indexes.insert(
            "user_id".to_string(),
            FieldIndex::Text(IndexKeyText {
                case_sensitive: false,
                alias: Some("uid".to_string()),
                chn: false,
                token: token_list![],
                doc_value: true,
            }),
        );

        let updated_full_text_index = FullTextIndex {
            case_sensitive: false,
            chn: true,
            token: token_list![",", " ", ";", "\n", "\t"],
        };

        let updated_index = Index::builder()
            .line(updated_full_text_index)
            .keys(updated_field_indexes)
            .build();

        let update_result = client
            .update_index(project, &logstore_name, updated_index)
            .send()
            .await;

        assert!(
            update_result.is_ok(),
            "Failed to update index: {:?}",
            update_result.err()
        );

        // Wait for update to take effect
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Step 4: Get index again and verify update
        println!("Getting updated index and verifying...");

        let get_updated_result = client.get_index(project, &logstore_name).send().await;

        assert!(
            get_updated_result.is_ok(),
            "Failed to get updated index: {:?}",
            get_updated_result.err()
        );

        let updated_response = get_updated_result.unwrap();
        let updated_retrieved_index = updated_response.get_body();

        let updated_keys = updated_retrieved_index.keys.as_ref().unwrap();
        assert!(
            updated_keys.contains_key("user_id"),
            "Should have 'user_id' field index after update"
        );

        println!("Index update verified successfully!");

        // Step 5: Delete index
        println!("Deleting index...");

        let delete_result = client.delete_index(project, &logstore_name).send().await;

        assert!(
            delete_result.is_ok(),
            "Failed to delete index: {:?}",
            delete_result.err()
        );

        // Wait for deletion to complete
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        // Step 6: Verify index is deleted (get should return error)
        println!("Verifying index deletion...");

        let get_after_delete = client.get_index(project, &logstore_name).send().await;

        assert!(
            get_after_delete.is_err(),
            "Getting deleted index should return error"
        );

        if let Err(aliyun_log_rust_sdk::Error::Server { error_code, .. }) = get_after_delete {
            assert_eq!(error_code, "IndexConfigNotExist");
        } else {
            panic!("Expected IndexConfigNotExist error after deletion");
        }

        println!("Index deletion verified successfully!");

        // Step 7: Clean up - delete the temporary logstore
        println!("Cleaning up temporary logstore...");

        let delete_logstore_result = client.delete_logstore(project, &logstore_name).send().await;

        assert!(
            delete_logstore_result.is_ok(),
            "Failed to delete temporary logstore: {:?}",
            delete_logstore_result.err()
        );

        println!("Test completed successfully!");
    }
}
