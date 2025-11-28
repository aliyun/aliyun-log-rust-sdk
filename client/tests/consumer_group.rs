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

    /// Macro to clean up consumer group at the start of test
    /// This ensures a clean state and catches any unexpected errors
    macro_rules! cleanup_consumer_group {
        ($client:expr, $project:expr, $logstore:expr, $consumer_group:expr) => {
            match $client
                .delete_consumer_group($project, $logstore, $consumer_group)
                .send()
                .await
            {
                Ok(_) => {} // Successfully deleted
                Err(e) => {
                    // Only ignore if consumer group doesn't exist
                    if !matches!(&e, aliyun_log_rust_sdk::Error::Server { error_code, .. } if error_code == "ConsumerGroupNotExist")
                    {
                        // For any other error, we should be aware of it but continue
                        eprintln!("Warning: Failed to cleanup consumer group: {}", e);
                    }
                }
            }
        };
    }

    /// Macro to create consumer group and handle AlreadyExist error
    macro_rules! create_consumer_group {
        ($client:expr, $project:expr, $logstore:expr, $consumer_group:expr, $timeout:expr, $order:expr) => {
            match $client
                .create_consumer_group($project, $logstore, $consumer_group)
                .timeout($timeout)
                .order($order)
                .send()
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    // Only accept ConsumerGroupAlreadyExist error
                    if !matches!(&e, aliyun_log_rust_sdk::Error::Server { error_code, .. } if error_code == "ConsumerGroupAlreadyExist")
                    {
                        panic!(
                            "Failed to create consumer group, expected success or ConsumerGroupAlreadyExist, got: {}",
                            e
                        );
                    }
                }
            }
        };
    }

    /// Check if error is a missing required parameter error with the expected parameter name
    fn is_missing_param_error(error: &aliyun_log_rust_sdk::Error, param_name: &str) -> bool {
        match error {
            aliyun_log_rust_sdk::Error::RequestPreparation(req_err) => {
                // Use Display format to get human-readable error message
                let err_msg = format!("{}", req_err);
                // Check for both "parameter: name" and "parameter \"name\"" formats
                err_msg.contains("Missing required parameter")
                    && (err_msg.contains(&format!(": {}", param_name))
                        || err_msg.contains(&format!("\"{}\"", param_name)))
            }
            _ => false,
        }
    }

    /// Assert that the result is a missing parameter error
    macro_rules! assert_missing_param {
        ($result:expr, $param:expr) => {
            match $result {
                Ok(_) => panic!(
                    "Expected missing parameter error for '{}', but operation succeeded",
                    $param
                ),
                Err(ref e) => {
                    assert!(
                        is_missing_param_error(e, $param),
                        "Expected missing parameter error for '{}', got: {}",
                        $param,
                        e // Use Display format, not Debug
                    );
                }
            }
        };
    }

    // Helper function to find a consumer group by name using list API
    async fn find_consumer_group(
        client: &Client,
        project: &str,
        logstore: &str,
        consumer_group_name: &str,
    ) -> Option<ConsumerGroup> {
        let resp = client
            .list_consumer_groups(project, logstore)
            .send()
            .await
            .unwrap();

        let consumer_groups = resp.get_body().consumer_groups();
        consumer_groups
            .iter()
            .find(|cg| cg.consumer_group_name() == consumer_group_name)
            .cloned()
    }

    #[tokio::test]
    async fn test_consumer_group_lifecycle() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-consumer-group-lifecycle";

        // Clean up any existing consumer group from previous test runs
        cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);

        // Test 1: Create consumer group
        create_consumer_group!(
            &TEST_CLIENT,
            project,
            logstore,
            consumer_group_name,
            30,
            true
        );
        println!("✓ Created consumer group: {consumer_group_name}");

        // Test 2: List consumer groups
        let resp = TEST_CLIENT
            .list_consumer_groups(project, logstore)
            .send()
            .await
            .unwrap();

        let consumer_groups = resp.get_body().consumer_groups();
        assert!(
            !consumer_groups.is_empty(),
            "Should have at least one consumer group"
        );

        let cg = consumer_groups
            .iter()
            .find(|cg| cg.consumer_group_name() == &consumer_group_name)
            .expect("Created consumer group should be in the list");

        assert_eq!(cg.consumer_group_name(), &consumer_group_name);
        assert_eq!(*cg.timeout(), 30);
        assert!(*cg.order());
        println!("✓ Listed consumer groups, found: {consumer_group_name}");

        // Test 3: Update consumer group
        let _ = TEST_CLIENT
            .update_consumer_group(project, logstore, &consumer_group_name)
            .timeout(60)
            .order(false)
            .send()
            .await
            .unwrap();
        println!("✓ Updated consumer group: {consumer_group_name}");

        // Verify update
        let cg = find_consumer_group(&TEST_CLIENT, project, logstore, &consumer_group_name)
            .await
            .expect("Consumer group should exist after update");

        assert_eq!(*cg.timeout(), 60);
        assert!(!(*cg.order()));
        println!("✓ Verified consumer group update");

        // Test 7: Delete consumer group
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, &consumer_group_name)
            .send()
            .await
            .unwrap();
        println!("✓ Deleted consumer group: {consumer_group_name}");

        // Verify deletion - consumer group should not exist anymore
        let resp = TEST_CLIENT
            .list_consumer_groups(project, logstore)
            .send()
            .await
            .unwrap();

        let consumer_groups = resp.get_body().consumer_groups();
        let still_exists = consumer_groups
            .iter()
            .any(|cg| cg.consumer_group_name() == &consumer_group_name);

        assert!(!still_exists, "Consumer group should be deleted");
        println!("✓ Verified consumer group deletion");
    }

    #[tokio::test]
    async fn test_consumer_group_error_handling() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;

        // Test finding non-existent consumer group
        let result = find_consumer_group(
            &TEST_CLIENT,
            project,
            logstore,
            "non-existent-consumer-group",
        )
        .await;

        match result {
            Some(_) => panic!("Consumer group should not exist"),
            None => println!("✓ Correctly handled non-existent consumer group - not found in list"),
        }

        // Test deleting non-existent consumer group
        let result = TEST_CLIENT
            .delete_consumer_group(project, logstore, "non-existent-consumer-group")
            .send()
            .await;

        // This might succeed or fail depending on the service behavior
        match result {
            Ok(_) => println!("✓ Delete non-existent consumer group succeeded"),
            Err(e) => println!("✓ Delete non-existent consumer group failed as expected: {e:?}"),
        }
    }

    #[tokio::test]
    async fn test_create_consumer_group_missing_parameters() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-create-missing-params-cg";

        // Clean up any existing consumer group
        cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);

        // Test 1: Missing timeout parameter
        let result = TEST_CLIENT
            .create_consumer_group(project, logstore, &consumer_group_name)
            // .timeout(30) - intentionally not set
            .order(true)
            .send()
            .await;

        assert_missing_param!(result, "timeout");

        // Test 2: Missing order parameter
        let result = TEST_CLIENT
            .create_consumer_group(project, logstore, &consumer_group_name)
            .timeout(30)
            // .order(true) - intentionally not set
            .send()
            .await;

        assert_missing_param!(result, "order");

        // Test 3: Both parameters missing (will fail on first missing param)
        let result = TEST_CLIENT
            .create_consumer_group(project, logstore, &consumer_group_name)
            // .timeout(30) - intentionally not set
            // .order(true) - intentionally not set
            .send()
            .await;

        // Will fail on first missing parameter checked (timeout or order)
        assert!(
            result.is_err(),
            "Create should have failed without required parameters"
        );
    }

    #[tokio::test]
    async fn test_update_consumer_group_missing_parameters() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-update-missing-params-cg";

        // Clean up and create a consumer group for testing
        cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);
        create_consumer_group!(
            &TEST_CLIENT,
            project,
            logstore,
            consumer_group_name,
            30,
            true
        );

        // Test 1: Missing timeout parameter
        let result = TEST_CLIENT
            .update_consumer_group(project, logstore, &consumer_group_name)
            // .timeout(60) - intentionally not set
            .order(false)
            .send()
            .await;

        assert_missing_param!(result, "timeout");

        // Test 2: Missing order parameter
        let result = TEST_CLIENT
            .update_consumer_group(project, logstore, &consumer_group_name)
            .timeout(60)
            // .order(false) - intentionally not set
            .send()
            .await;

        assert_missing_param!(result, "order");

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, &consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_consumer_group_heartbeat_missing_parameters() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-heartbeat-missing-params-cg";

        // Clean up and create a consumer group for testing
        cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);
        create_consumer_group!(
            &TEST_CLIENT,
            project,
            logstore,
            consumer_group_name,
            30,
            true
        );

        // Test: Missing consumer parameter
        let result = TEST_CLIENT
            .consumer_group_heartbeat(project, logstore, &consumer_group_name)
            // .consumer("test-consumer") - intentionally not set
            .send()
            .await;

        assert_missing_param!(result, "consumer");

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, &consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_update_checkpoint_missing_parameters() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-checkpoint-missing-params-cg";

        // Clean up and create a consumer group for testing
        cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);
        create_consumer_group!(
            &TEST_CLIENT,
            project,
            logstore,
            consumer_group_name,
            30,
            true
        );

        // Test 1: Missing shard_id parameter
        let result = TEST_CLIENT
            .update_consumer_group_checkpoint(project, logstore, &consumer_group_name)
            // .shard_id(0) - intentionally not set
            .consumer_id("test-consumer")
            .checkpoint("test-cursor")
            .send()
            .await;

        assert_missing_param!(result, "shard_id");

        // Test 2: Missing consumer_id parameter
        let result = TEST_CLIENT
            .update_consumer_group_checkpoint(project, logstore, &consumer_group_name)
            .shard_id(0)
            // .consumer_id("test-consumer") - intentionally not set
            .checkpoint("test-cursor")
            .send()
            .await;

        assert_missing_param!(result, "consumer_id");

        // Test 3: Missing checkpoint parameter
        let result = TEST_CLIENT
            .update_consumer_group_checkpoint(project, logstore, &consumer_group_name)
            .shard_id(0)
            .consumer_id("test-consumer")
            // .checkpoint("test-cursor") - intentionally not set
            .send()
            .await;

        assert_missing_param!(result, "checkpoint");

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, &consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_parameter_validation_edge_cases() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;

        // Test: Empty consumer group name
        let result = TEST_CLIENT
            .create_consumer_group(project, logstore, "")
            .timeout(30)
            .order(true)
            .send()
            .await;

        // This should either succeed or fail gracefully
        match result {
            Ok(_) => println!("✓ Create with empty consumer group name succeeded"),
            Err(e) => println!("✓ Create with empty consumer group name failed gracefully: {e:?}"),
        }

        // Test: Very long consumer group name
        let long_name = "a".repeat(200);
        let result = TEST_CLIENT
            .create_consumer_group(project, logstore, &long_name)
            .timeout(30)
            .order(true)
            .send()
            .await;

        match result {
            Ok(_) => println!("✓ Create with very long consumer group name succeeded"),
            Err(e) => println!("✓ Create with very long consumer group name failed: {e:?}"),
        }
    }

    #[tokio::test]
    async fn test_consumer_group_heartbeat() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-heartbeat-cg";

        // Clean up and create a consumer group for testing heartbeat
        cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);
        create_consumer_group!(
            &TEST_CLIENT,
            project,
            logstore,
            consumer_group_name,
            30,
            true
        );

        // Test heartbeat
        let result = TEST_CLIENT
            .consumer_group_heartbeat(project, logstore, &consumer_group_name)
            .consumer("test-consumer-1")
            .send()
            .await;

        result.expect("Failed to send heartbeat");

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, &consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_get_empty_checkpoints() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-empty-checkpoints-cg";

        // Clean up and create a new consumer group
        cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);
        create_consumer_group!(
            &TEST_CLIENT,
            project,
            logstore,
            consumer_group_name,
            30,
            true
        );

        // Get checkpoints - should be empty initially
        // We just verify we can get the response successfully
        TEST_CLIENT
            .get_consumer_group_checkpoint(project, logstore, &consumer_group_name)
            .send()
            .await
            .expect("Failed to get checkpoints");

        // Initially, there should be no checkpoints or empty checkpoints
        // The actual behavior depends on the service implementation

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, &consumer_group_name)
            .send()
            .await;
    }

    // #[tokio::test]
    // async fn test_checkpoint_workflow_with_update() {
    //     init();
    //     let project = &crate::tests::TEST_ENV.project;
    //     let logstore = &crate::tests::TEST_ENV.logstore;
    //     let consumer_group_name = "test-checkpoint-workflow-cg";
    //     let consumer_id_1 = "consumer-1";
    //     let consumer_id_2 = "consumer-2";

    //     // Clean up and create a consumer group
    //     cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);
    //     create_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name, 30, true);

    //     // Get shards to work with
    //     let shards_resp = TEST_CLIENT
    //         .list_shards(project, logstore)
    //         .send()
    //         .await
    //         .unwrap();

    //     let shards = shards_resp.get_body().shards();
    //     assert!(!shards.is_empty(), "Logstore should have at least one shard");

    //     // Test with first shard
    //     let shard_id_1 = *shards[0].shard_id();
    //     let shard_id_2 = if shards.len() > 1 {
    //         *shards[1].shard_id()
    //     } else {
    //         shard_id_1
    //     };

    //     // Get initial cursor for first shard
    //     let cursor_resp_1 = TEST_CLIENT
    //         .get_cursor(project, logstore, shard_id_1)
    //         .cursor_pos(aliyun_log_rust_sdk::get_cursor_models::CursorPos::Begin)
    //         .send()
    //         .await
    //         .unwrap();
    //     let cursor_1 = cursor_resp_1.get_body().cursor();

    //     // Get initial cursor for second shard (if different)
    //     let cursor_resp_2 = TEST_CLIENT
    //         .get_cursor(project, logstore, shard_id_2)
    //         .cursor_pos(aliyun_log_rust_sdk::get_cursor_models::CursorPos::Begin)
    //         .send()
    //         .await
    //         .unwrap();
    //     let cursor_2 = cursor_resp_2.get_body().cursor();

    //     // Update checkpoint for first consumer on first shard
    //     let update_result_1 = TEST_CLIENT
    //         .update_consumer_group_checkpoint(project, logstore, &consumer_group_name)
    //         .shard_id(shard_id_1)
    //         .consumer_id(consumer_id_1)
    //         .checkpoint(cursor_1)
    //         .force_success(true)
    //         .send()
    //         .await;

    //     match update_result_1 {
    //         Ok(_) => {} // Successfully updated
    //         Err(e) => {
    //             panic!("Failed to update checkpoint for consumer {consumer_id_1}: {e:?}");
    //         }
    //     }

    //     // Update checkpoint for second consumer on second shard
    //     if shard_id_1 != shard_id_2 {
    //         let update_result_2 = TEST_CLIENT
    //             .update_consumer_group_checkpoint(project, logstore, &consumer_group_name)
    //             .shard_id(shard_id_2)
    //             .consumer_id(consumer_id_2)
    //             .checkpoint(cursor_2)
    //             .force_success(true)
    //             .send()
    //             .await;

    //         match update_result_2 {
    //             Ok(_) => {} // Successfully updated
    //             Err(e) => {
    //                 panic!("Failed to update checkpoint for consumer {consumer_id_2}: {e:?}");
    //             }
    //         }
    //     }

    //     // Wait a moment for checkpoints to be updated
    //     tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    //     // Get checkpoints
    //     let resp = TEST_CLIENT
    //         .get_consumer_group_checkpoint(project, logstore, &consumer_group_name)
    //         .send()
    //         .await;

    //     let checkpoints_resp = resp.expect("Failed to get checkpoints");
    //     let checkpoints = checkpoints_resp.get_body().checkpoints();

    //     // Verify we have checkpoints for the consumers we updated
    //     let consumer_1_found = checkpoints
    //         .iter()
    //         .any(|cp| cp.consumer() == consumer_id_1 && *cp.shard_id() == shard_id_1);

    //     assert!(
    //         consumer_1_found,
    //         "Checkpoint for consumer {consumer_id_1} on shard {shard_id_1} should exist"
    //     );

    //     if shard_id_1 != shard_id_2 {
    //         let consumer_2_found = checkpoints
    //             .iter()
    //             .any(|cp| cp.consumer() == consumer_id_2 && *cp.shard_id() == shard_id_2);
    //         assert!(
    //             consumer_2_found,
    //             "Checkpoint for consumer {consumer_id_2} on shard {shard_id_2} should exist"
    //         );
    //     }

    //     // Clean up
    //     let _ = TEST_CLIENT
    //         .delete_consumer_group(project, logstore, &consumer_group_name)
    //         .send()
    //         .await;
    // }

    #[tokio::test]
    async fn test_get_checkpoint_nonexistent_consumer_group() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let nonexistent_cg = "nonexistent-consumer-group-12345";

        // Try to get checkpoints for non-existent consumer group
        let result = TEST_CLIENT
            .get_consumer_group_checkpoint(project, logstore, nonexistent_cg)
            .send()
            .await;

        assert!(
            result.is_err(),
            "Should fail to get checkpoints for non-existent consumer group"
        );
    }

    // #[tokio::test]
    // async fn test_checkpoint_update_and_verify() {
    //     init();
    //     let project = &crate::tests::TEST_ENV.project;
    //     let logstore = &crate::tests::TEST_ENV.logstore;
    //     let consumer_group_name = "test-verify-checkpoint-cg";
    //     let consumer_id = "test-consumer";

    //     // Clean up and create consumer group
    //     cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);
    //     create_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name, 30, true);

    //     // Get shards
    //     let shards_resp = TEST_CLIENT
    //         .list_shards(project, logstore)
    //         .send()
    //         .await
    //         .unwrap();

    //     let shards = shards_resp.get_body().shards();
    //     assert!(!shards.is_empty(), "Logstore should have at least one shard");

    //     let shard_id = *shards[0].shard_id();

    //     // Get different cursors to simulate progress
    //     let cursor_resp = TEST_CLIENT
    //         .get_cursor(project, logstore, shard_id)
    //         .cursor_pos(aliyun_log_rust_sdk::get_cursor_models::CursorPos::Begin)
    //         .send()
    //         .await
    //         .unwrap();
    //     let cursor_begin = cursor_resp.get_body().cursor().to_string();

    //     // Update checkpoint with initial cursor
    //     let _ = TEST_CLIENT
    //         .update_consumer_group_checkpoint(project, logstore, &consumer_group_name)
    //         .shard_id(shard_id)
    //         .consumer_id(consumer_id)
    //         .checkpoint(cursor_begin.clone())
    //         .force_success(true)
    //         .send()
    //         .await;

    //     // Wait and get checkpoint
    //     tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    //     let checkpoints_resp = TEST_CLIENT
    //         .get_consumer_group_checkpoint(project, logstore, &consumer_group_name)
    //         .send()
    //         .await
    //         .unwrap();

    //     let checkpoints = checkpoints_resp.get_body().checkpoints();

    //     // Find and verify our checkpoint
    //     let checkpoint = checkpoints
    //         .iter()
    //         .find(|cp| cp.consumer() == consumer_id && *cp.shard_id() == shard_id)
    //         .expect(&format!(
    //             "Checkpoint should exist for consumer {consumer_id} on shard {shard_id}"
    //         ));

    //     assert_eq!(checkpoint.consumer(), consumer_id);
    //     assert_eq!(*checkpoint.shard_id(), shard_id);
    //     assert!(!checkpoint.checkpoint().is_empty());

    //     // Clean up
    //     let _ = TEST_CLIENT
    //         .delete_consumer_group(project, logstore, &consumer_group_name)
    //         .send()
    //         .await;
    // }

    // #[tokio::test]
    // async fn test_update_consumer_group_checkpoint() {
    //     init();
    //     let project = &crate::tests::TEST_ENV.project;
    //     let logstore = &crate::tests::TEST_ENV.logstore;
    //     let consumer_group_name = "test-checkpoint-cg";

    //     // Clean up and create a consumer group
    //     cleanup_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name);
    //     create_consumer_group!(&TEST_CLIENT, project, logstore, consumer_group_name, 30, true);

    //     // Get shards to test checkpoint
    //     let shards_resp = TEST_CLIENT
    //         .list_shards(project, logstore)
    //         .send()
    //         .await
    //         .unwrap();

    //     let shards = shards_resp.get_body().shards();
    //     assert!(!shards.is_empty(), "Logstore should have at least one shard");

    //     let shard_id = shards[0].shard_id();

    //     // Get a cursor for the shard
    //     let cursor_resp = TEST_CLIENT
    //         .get_cursor(project, logstore, *shard_id)
    //         .cursor_pos(aliyun_log_rust_sdk::get_cursor_models::CursorPos::Begin)
    //         .send()
    //         .await
    //         .unwrap();

    //     let cursor = cursor_resp.get_body().cursor();

    //     TEST_CLIENT
    //         .update_consumer_group_checkpoint(project, logstore, &consumer_group_name)
    //         .shard_id(*shard_id)
    //         .consumer_id("my-consumer-id")
    //         .checkpoint(cursor)
    //         .force_success(true)
    //         .send()
    //         .await
    //         .expect("Failed to update checkpoint");

    //     // Clean up
    //     let _ = TEST_CLIENT
    //         .delete_consumer_group(project, logstore, &consumer_group_name)
    //         .send()
    //         .await;
    // }
}
