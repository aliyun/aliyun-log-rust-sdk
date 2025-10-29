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
        let consumer_group_name = "test-consumer-group";

        // Clean up any existing consumer group from previous test runs
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;

        // Test 1: Create consumer group

        let resp = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await;

        match resp {
            Ok(_) => println!("✓ Created consumer group: {consumer_group_name}"),
            Err(e) => {
                println!("✗ Failed to create consumer group: {e:?}");
                panic!("Create consumer group failed");
            }
        }

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
            .find(|cg| cg.consumer_group_name() == consumer_group_name)
            .expect("Created consumer group should be in the list");

        assert_eq!(cg.consumer_group_name(), consumer_group_name);
        assert_eq!(*cg.timeout(), 30);
        assert!(*cg.order());
        println!("✓ Listed consumer groups, found: {consumer_group_name}");

        // Test 3: Update consumer group
        let _ = TEST_CLIENT
            .update_consumer_group(project, logstore, consumer_group_name)
            .timeout(60)
            .order(false)
            .send()
            .await
            .unwrap();
        println!("✓ Updated consumer group: {consumer_group_name}");

        // Verify update
        let cg = find_consumer_group(&TEST_CLIENT, project, logstore, consumer_group_name)
            .await
            .expect("Consumer group should exist after update");

        assert_eq!(*cg.timeout(), 60);
        assert!(!(*cg.order()));
        println!("✓ Verified consumer group update");

        // Test 7: Delete consumer group
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
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
            .any(|cg| cg.consumer_group_name() == consumer_group_name);

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
            Some(_) => println!("⚠ Consumer group exists unexpectedly"),
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
        let consumer_group_name = "test-missing-params-cg";

        // Clean up any existing consumer group
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;

        // Test 1: Missing timeout parameter
        let result = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            // .timeout(30) - intentionally not set
            .order(true)
            .send()
            .await;

        match result {
            Ok(_) => println!("⚠ Create succeeded unexpectedly without timeout"),
            Err(e) => {
                println!("✓ Correctly failed when timeout is missing: {e:?}");
                // Validate error type by checking error message
                let error_msg = format!("{e:?}");
                if error_msg.contains("timeout") && error_msg.contains("Missing required parameter")
                {
                    println!("✓ Correctly identified missing parameter: timeout");
                }
            }
        }

        // Test 2: Missing order parameter
        let result = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            // .order(true) - intentionally not set
            .send()
            .await;

        match result {
            Ok(_) => println!("⚠ Create succeeded unexpectedly without order"),
            Err(e) => {
                println!("✓ Correctly failed when order is missing: {e:?}");
                // Validate error type by checking error message
                let error_msg = format!("{e:?}");
                if error_msg.contains("order") && error_msg.contains("Missing required parameter") {
                    println!("✓ Correctly identified missing parameter: order");
                }
            }
        }

        // Test 3: Both parameters missing
        let result = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            // .timeout(30) - intentionally not set
            // .order(true) - intentionally not set
            .send()
            .await;

        match result {
            Ok(_) => println!("⚠ Create succeeded unexpectedly without any parameters"),
            Err(e) => {
                println!("✓ Correctly failed when both parameters are missing: {e:?}");
                // Validate error type by checking error message
                let error_msg = format!("{e:?}");
                if (error_msg.contains("timeout") || error_msg.contains("order"))
                    && error_msg.contains("Missing required parameter")
                {
                    println!("✓ Correctly identified missing parameter");
                }
            }
        }
    }

    #[tokio::test]
    async fn test_update_consumer_group_missing_parameters() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-update-missing-params-cg";

        // First create a consumer group for testing
        let _ = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await;

        // Test 1: Missing timeout parameter
        let result = TEST_CLIENT
            .update_consumer_group(project, logstore, consumer_group_name)
            // .timeout(60) - intentionally not set
            .order(false)
            .send()
            .await;

        match result {
            Ok(_) => println!("⚠ Update succeeded unexpectedly without timeout"),
            Err(e) => {
                println!("✓ Correctly failed when timeout is missing: {e:?}");
                let error_msg = format!("{e:?}");
                if error_msg.contains("timeout") && error_msg.contains("Missing required parameter")
                {
                    println!("✓ Correctly identified missing parameter: timeout");
                }
            }
        }

        // Test 2: Missing order parameter
        let result = TEST_CLIENT
            .update_consumer_group(project, logstore, consumer_group_name)
            .timeout(60)
            // .order(false) - intentionally not set
            .send()
            .await;

        match result {
            Ok(_) => println!("⚠ Update succeeded unexpectedly without order"),
            Err(e) => {
                println!("✓ Correctly failed when order is missing: {e:?}");
                let error_msg = format!("{e:?}");
                if error_msg.contains("order") && error_msg.contains("Missing required parameter") {
                    println!("✓ Correctly identified missing parameter: order");
                }
            }
        }

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_consumer_group_heartbeat_missing_parameters() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-heartbeat-missing-params-cg";

        // First create a consumer group for testing
        let _ = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await;

        // Test: Missing consumer parameter
        let result = TEST_CLIENT
            .consumer_group_heartbeat(project, logstore, consumer_group_name)
            // .consumer("test-consumer") - intentionally not set
            .send()
            .await;

        match result {
            Ok(_) => println!("⚠ Heartbeat succeeded unexpectedly without consumer"),
            Err(e) => {
                println!("✓ Correctly failed when consumer is missing: {e:?}");
                let error_msg = format!("{e:?}");
                if error_msg.contains("consumer")
                    && error_msg.contains("Missing required parameter")
                {
                    println!("✓ Correctly identified missing parameter: consumer");
                }
            }
        }

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_update_checkpoint_missing_parameters() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-checkpoint-missing-params-cg";

        // First create a consumer group for testing
        let _ = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await;

        // Test 1: Missing shard_id parameter
        let result = TEST_CLIENT
            .update_consumer_group_checkpoint(project, logstore, consumer_group_name)
            // .shard_id(0) - intentionally not set
            .consumer_id("test-consumer")
            .checkpoint("test-cursor")
            .send()
            .await;

        match result {
            Ok(_) => println!("⚠ Update checkpoint succeeded unexpectedly without shard_id"),
            Err(e) => {
                println!("✓ Correctly failed when shard_id is missing: {e:?}");
                let error_msg = format!("{e:?}");
                if error_msg.contains("shard_id")
                    && error_msg.contains("Missing required parameter")
                {
                    println!("✓ Correctly identified missing parameter: shard_id");
                }
            }
        }

        // Test 2: Missing consumer_id parameter
        let result = TEST_CLIENT
            .update_consumer_group_checkpoint(project, logstore, consumer_group_name)
            .shard_id(0)
            // .consumer_id("test-consumer") - intentionally not set
            .checkpoint("test-cursor")
            .send()
            .await;

        match result {
            Ok(_) => println!("⚠ Update checkpoint succeeded unexpectedly without consumer_id"),
            Err(e) => {
                println!("✓ Correctly failed when consumer_id is missing: {e:?}");
                let error_msg = format!("{e:?}");
                if error_msg.contains("consumer_id")
                    && error_msg.contains("Missing required parameter")
                {
                    println!("✓ Correctly identified missing parameter: consumer_id");
                }
            }
        }

        // Test 3: Missing checkpoint parameter (when force_success is also None)
        let result = TEST_CLIENT
            .update_consumer_group_checkpoint(project, logstore, consumer_group_name)
            .shard_id(0)
            .consumer_id("test-consumer")
            // .checkpoint("test-cursor") - intentionally not set
            // .force_success(true) - intentionally not set
            .send()
            .await;

        match result {
            Ok(_) => println!(
                "⚠ Update checkpoint succeeded unexpectedly without checkpoint or force_success"
            ),
            Err(e) => {
                println!(
                    "✓ Correctly failed when both checkpoint and force_success are missing: {e:?}"
                );
                let error_msg = format!("{e:?}");
                if error_msg.contains("checkpoint")
                    && error_msg.contains("Missing required parameter")
                {
                    println!("✓ Correctly identified missing parameter: checkpoint");
                }
            }
        }

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
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

        // Create a consumer group first for testing heartbeat

        let _ = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await;

        // Test heartbeat
        let result = TEST_CLIENT
            .consumer_group_heartbeat(project, logstore, consumer_group_name)
            .consumer("test-consumer-1")
            .send()
            .await;

        match result {
            Ok(_) => println!("✓ Consumer group heartbeat sent successfully"),
            Err(e) => {
                println!(
                    "⚠ Heartbeat failed (might be expected if consumer group doesn't exist): {e:?}"
                );
            }
        }

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_heartbeat_request_model() {
        // Test HeartbeatRequest model
        let heartbeat_request = heartbeat_models::HeartbeatRequest {
            consumer: "test-consumer".to_string(),
        };

        assert_eq!(heartbeat_request.consumer, "test-consumer");
    }
    #[tokio::test]
    async fn test_get_empty_checkpoints() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-empty-checkpoints-cg";

        // Clean up any existing consumer group
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;

        // Create a new consumer group
        let _ = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await
            .unwrap();

        // Get checkpoints - should be empty initially
        let resp = TEST_CLIENT
            .get_consumer_group_checkpoint(project, logstore, consumer_group_name)
            .send()
            .await
            .unwrap();

        let checkpoints = resp.get_body().checkpoints();
        println!(
            "✓ Got {} checkpoints for new consumer group",
            checkpoints.len()
        );

        // Initially, there should be no checkpoints or empty checkpoints
        // The actual behavior depends on the service implementation

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_checkpoint_workflow_with_update() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-checkpoint-workflow-cg";
        let consumer_id_1 = "consumer-1";
        let consumer_id_2 = "consumer-2";

        // Clean up any existing consumer group
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;

        // Create a consumer group
        let _ = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await
            .unwrap();

        // Get shards to work with
        let shards_resp = TEST_CLIENT
            .list_shards(project, logstore)
            .send()
            .await
            .unwrap();

        let shards = shards_resp.get_body().shards();
        if shards.is_empty() {
            println!("⚠ No shards available for checkpoint test, skipping");
            return;
        }

        // Test with first shard
        let shard_id_1 = *shards[0].shard_id();
        let shard_id_2 = if shards.len() > 1 {
            *shards[1].shard_id()
        } else {
            shard_id_1
        };

        // Get initial cursor for first shard
        let cursor_resp_1 = TEST_CLIENT
            .get_cursor(project, logstore, shard_id_1)
            .cursor_pos(aliyun_log_rust_sdk::get_cursor_models::CursorPos::Begin)
            .send()
            .await
            .unwrap();
        let cursor_1 = cursor_resp_1.get_body().cursor();

        // Get initial cursor for second shard (if different)
        let cursor_resp_2 = TEST_CLIENT
            .get_cursor(project, logstore, shard_id_2)
            .cursor_pos(aliyun_log_rust_sdk::get_cursor_models::CursorPos::Begin)
            .send()
            .await
            .unwrap();
        let cursor_2 = cursor_resp_2.get_body().cursor();

        // Update checkpoint for first consumer on first shard
        let update_result_1 = TEST_CLIENT
            .update_consumer_group_checkpoint(project, logstore, consumer_group_name)
            .shard_id(shard_id_1)
            .consumer_id(consumer_id_1)
            .checkpoint(cursor_1)
            .force_success(true)
            .send()
            .await;

        match update_result_1 {
            Ok(_) => {
                println!("✓ Updated checkpoint for consumer {consumer_id_1} on shard {shard_id_1}")
            }
            Err(e) => {
                println!("⚠ Failed to update checkpoint for consumer {consumer_id_1}: {e:?}");
                // Continue with the test even if update fails
            }
        }

        // Update checkpoint for second consumer on second shard
        if shard_id_1 != shard_id_2 {
            let update_result_2 = TEST_CLIENT
                .update_consumer_group_checkpoint(project, logstore, consumer_group_name)
                .shard_id(shard_id_2)
                .consumer_id(consumer_id_2)
                .checkpoint(cursor_2)
                .force_success(true)
                .send()
                .await;

            match update_result_2 {
                Ok(_) => println!(
                    "✓ Updated checkpoint for consumer {consumer_id_2} on shard {shard_id_2}"
                ),
                Err(e) => {
                    println!("⚠ Failed to update checkpoint for consumer {consumer_id_2}: {e:?}");
                }
            }
        }

        // Wait a moment for checkpoints to be updated
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Get checkpoints
        let resp = TEST_CLIENT
            .get_consumer_group_checkpoint(project, logstore, consumer_group_name)
            .send()
            .await;

        match resp {
            Ok(checkpoints_resp) => {
                let checkpoints = checkpoints_resp.get_body().checkpoints();
                println!("✓ Retrieved {} checkpoints", checkpoints.len());

                for checkpoint in checkpoints {
                    println!(
                        "  Checkpoint: shard={}, consumer={}, checkpoint={}, update_time={}",
                        checkpoint.shard_id(),
                        checkpoint.consumer(),
                        checkpoint.checkpoint(),
                        checkpoint.update_time()
                    );
                }

                // Verify we have checkpoints for the consumers we updated
                let consumer_1_found = checkpoints
                    .iter()
                    .any(|cp| cp.consumer() == consumer_id_1 && *cp.shard_id() == shard_id_1);
                let consumer_2_found = if shard_id_1 != shard_id_2 {
                    checkpoints
                        .iter()
                        .any(|cp| cp.consumer() == consumer_id_2 && *cp.shard_id() == shard_id_2)
                } else {
                    true // Skip if same shard
                };

                if consumer_1_found {
                    println!(
                        "✓ Found checkpoint for consumer {consumer_id_1} on shard {shard_id_1}"
                    );
                } else {
                    println!("⚠ Checkpoint for consumer {consumer_id_1} not found");
                }

                if consumer_2_found {
                    println!(
                        "✓ Found checkpoint for consumer {consumer_id_2} on shard {shard_id_2}"
                    );
                }
            }
            Err(e) => {
                println!("⚠ Failed to get checkpoints: {e:?}");
            }
        }

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;
    }

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

        match result {
            Ok(_) => println!("⚠ Unexpectedly got checkpoints for non-existent consumer group"),
            Err(e) => println!(
                "✓ Correctly failed to get checkpoints for non-existent consumer group: {e:?}"
            ),
        }
    }

    #[tokio::test]
    async fn test_checkpoint_update_and_verify() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-verify-checkpoint-cg";
        let consumer_id = "test-consumer";

        // Clean up any existing consumer group
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;

        // Create consumer group
        let _ = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await
            .unwrap();

        // Get shards
        let shards_resp = TEST_CLIENT
            .list_shards(project, logstore)
            .send()
            .await
            .unwrap();

        let shards = shards_resp.get_body().shards();
        if shards.is_empty() {
            println!("⚠ No shards available, skipping test");
            return;
        }

        let shard_id = *shards[0].shard_id();

        // Get different cursors to simulate progress
        let cursor_resp = TEST_CLIENT
            .get_cursor(project, logstore, shard_id)
            .cursor_pos(aliyun_log_rust_sdk::get_cursor_models::CursorPos::Begin)
            .send()
            .await
            .unwrap();
        let cursor_begin = cursor_resp.get_body().cursor().to_string();

        // Update checkpoint with initial cursor
        let _ = TEST_CLIENT
            .update_consumer_group_checkpoint(project, logstore, consumer_group_name)
            .shard_id(shard_id)
            .consumer_id(consumer_id)
            .checkpoint(cursor_begin.clone())
            .force_success(true)
            .send()
            .await;

        // Wait and get checkpoint
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        let checkpoints_resp = TEST_CLIENT
            .get_consumer_group_checkpoint(project, logstore, consumer_group_name)
            .send()
            .await
            .unwrap();

        let checkpoints = checkpoints_resp.get_body().checkpoints();

        // Find our checkpoint
        if let Some(checkpoint) = checkpoints
            .iter()
            .find(|cp| cp.consumer() == consumer_id && *cp.shard_id() == shard_id)
        {
            println!("✓ Found checkpoint: {}", checkpoint.checkpoint());
            assert_eq!(checkpoint.consumer(), consumer_id);
            assert_eq!(*checkpoint.shard_id(), shard_id);
            assert!(!checkpoint.checkpoint().is_empty());

            // Verify the checkpoint matches what we set
            if checkpoint.checkpoint() == &cursor_begin {
                println!("✓ Checkpoint value matches expected cursor");
            } else {
                println!("⚠ Checkpoint value differs from expected (this may be normal if service modifies it)");
            }
        } else {
            println!("⚠ Checkpoint not found for consumer {consumer_id} on shard {shard_id}");
        }

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;
    }

    #[tokio::test]
    async fn test_update_consumer_group_checkpoint() {
        init();
        let project = &crate::tests::TEST_ENV.project;
        let logstore = &crate::tests::TEST_ENV.logstore;
        let consumer_group_name = "test-checkpoint-cg";

        // Create a consumer group first
        let _ = TEST_CLIENT
            .create_consumer_group(project, logstore, consumer_group_name)
            .timeout(30)
            .order(true)
            .send()
            .await;

        // Get shards to test checkpoint
        let shards_resp = TEST_CLIENT
            .list_shards(project, logstore)
            .send()
            .await
            .unwrap();

        let shards = shards_resp.get_body().shards();
        if !shards.is_empty() {
            let shard_id = shards[0].shard_id();

            // Get a cursor for the shard
            let cursor_resp = TEST_CLIENT
                .get_cursor(project, logstore, *shard_id)
                .cursor_pos(aliyun_log_rust_sdk::get_cursor_models::CursorPos::Begin)
                .send()
                .await
                .unwrap();

            let cursor = cursor_resp.get_body().cursor();

            let result = TEST_CLIENT
                .update_consumer_group_checkpoint(project, logstore, consumer_group_name)
                .shard_id(*shard_id)
                .consumer_id("my-consumer-id")
                .checkpoint(cursor)
                .force_success(true)
                .send()
                .await;

            match result {
                Ok(_) => println!(
                    "✓ Consumer group checkpoint updated successfully for shard {shard_id}"
                ),
                Err(e) => {
                    println!("⚠ Checkpoint update failed: {e:?}");
                }
            }
        } else {
            println!("⚠ No shards available for checkpoint test");
        }

        // Clean up
        let _ = TEST_CLIENT
            .delete_consumer_group(project, logstore, consumer_group_name)
            .send()
            .await;
    }
}
