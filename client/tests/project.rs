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

    /// Get test project name with suffix
    fn get_test_project_name() -> String {
        format!("{}-for-test", TEST_ENV.project)
    }

    /// Macro to clean up project at the start of test
    macro_rules! cleanup_project {
        ($client:expr, $project_name:expr) => {
            match $client.delete_project($project_name).send().await {
                Ok(_) => {
                    // Wait a moment for project deletion to complete
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                Err(e) => {
                    // Only ignore if project doesn't exist
                    if !matches!(&e, aliyun_log_rust_sdk::Error::Server { error_code, .. } if error_code == "ProjectNotExist")
                    {
                        eprintln!("Warning: Failed to cleanup project: {}", e);
                    }
                }
            }
        };
    }

    /// Macro to create project and handle AlreadyExist error
    macro_rules! create_project {
        ($client:expr, $project_name:expr, $description:expr) => {
            match $client
                .create_project($project_name)
                .description($description)
                .send()
                .await
            {
                Ok(_) => {
                    // Wait a moment for project creation to complete
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
                Err(e) => {
                    // Only accept ProjectAlreadyExist error
                    if !matches!(&e, aliyun_log_rust_sdk::Error::Server { error_code, .. } if error_code == "ProjectAlreadyExist")
                    {
                        panic!(
                            "Failed to create project, expected success or ProjectAlreadyExist, got: {}",
                            e
                        );
                    }
                }
            }
        };
    }

    /// Check if error is a missing required parameter error
    fn is_missing_param_error(error: &aliyun_log_rust_sdk::Error, param_name: &str) -> bool {
        match error {
            aliyun_log_rust_sdk::Error::RequestPreparation(req_err) => {
                let err_msg = format!("{}", req_err);
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
                        e
                    );
                }
            }
        };
    }

    #[tokio::test]
    async fn test_project_lifecycle() {
        init();
        let project_name = get_test_project_name();

        // Clean up any existing project from previous test runs
        cleanup_project!(&TEST_CLIENT, &project_name);

        // Test 1: Create project
        create_project!(&TEST_CLIENT, &project_name, "Test project for testing");
        println!("✓ Created project: {}", project_name);

        // Test 2: Get project
        let get_resp = TEST_CLIENT.get_project(&project_name).send().await.unwrap();

        let project_info = get_resp.get_body();
        assert_eq!(project_info.project_name(), &project_name);
        assert_eq!(project_info.description(), "Test project for testing");
        assert_eq!(project_info.status(), "Normal");
        println!("✓ Got project: {}", project_name);

        // Test 3: List projects
        let list_resp = TEST_CLIENT
            .list_projects(0, 100)
            .project_name(&project_name)
            .send()
            .await
            .unwrap();

        let projects = list_resp.get_body().projects();
        let found_project = projects.iter().find(|p| p.project_name() == &project_name);

        assert!(
            found_project.is_some(),
            "Created project should be in the list"
        );
        println!("✓ Listed projects, found: {}", project_name);

        // Test 4: Update project
        TEST_CLIENT
            .update_project(&project_name)
            .description("Updated test project description")
            .recycle_bin_enabled(true)
            .send()
            .await
            .unwrap();
        println!("✓ Updated project: {}", project_name);

        // Verify update
        let get_resp = TEST_CLIENT.get_project(&project_name).send().await.unwrap();

        let project_info = get_resp.get_body();
        assert_eq!(
            project_info.description(),
            "Updated test project description"
        );
        if let Some(recycle_bin_enabled) = project_info.recycle_bin_enabled() {
            assert!(*recycle_bin_enabled);
        }
        println!("✓ Verified project update");

        // Test 5: Delete project
        TEST_CLIENT
            .delete_project(&project_name)
            .send()
            .await
            .unwrap();
        println!("✓ Deleted project: {}", project_name);

        // Wait a moment for deletion to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Verify deletion
        let get_result = TEST_CLIENT.get_project(&project_name).send().await;

        assert!(
            get_result.is_err(),
            "Project should not exist after deletion"
        );
        println!("✓ Verified project deletion");
    }

    #[tokio::test]
    async fn test_project_error_handling() {
        init();
        let non_existent_project = format!("{}-non-exist", get_test_project_name());

        // Test 1: Get non-existent project
        let result = TEST_CLIENT.get_project(&non_existent_project).send().await;

        assert!(result.is_err(), "Should fail to get non-existent project");

        // Verify it's a ProjectNotExist error
        if let Err(e) = result {
            match &e {
                aliyun_log_rust_sdk::Error::Server { error_code, .. } => {
                    assert_eq!(
                        error_code, "ProjectNotExist",
                        "Should return ProjectNotExist error"
                    );
                }
                _ => panic!("Expected Server error with ProjectNotExist, got: {}", e),
            }
        }
        println!("✓ Correctly handled non-existent project in get");

        // Test 2: Update non-existent project
        let result = TEST_CLIENT
            .update_project(&non_existent_project)
            .description("test")
            .send()
            .await;

        assert!(
            result.is_err(),
            "Should fail to update non-existent project"
        );

        if let Err(e) = result {
            match &e {
                aliyun_log_rust_sdk::Error::Server { error_code, .. } => {
                    assert_eq!(
                        error_code, "ProjectNotExist",
                        "Should return ProjectNotExist error"
                    );
                }
                _ => panic!("Expected Server error with ProjectNotExist, got: {}", e),
            }
        }
        println!("✓ Correctly handled non-existent project in update");

        // Test 3: Delete non-existent project
        let result = TEST_CLIENT
            .delete_project(&non_existent_project)
            .send()
            .await;

        assert!(
            result.is_err(),
            "Should fail to delete non-existent project"
        );

        if let Err(e) = result {
            match &e {
                aliyun_log_rust_sdk::Error::Server { error_code, .. } => {
                    assert_eq!(
                        error_code, "ProjectNotExist",
                        "Should return ProjectNotExist error"
                    );
                }
                _ => panic!("Expected Server error with ProjectNotExist, got: {}", e),
            }
        }
        println!("✓ Correctly handled non-existent project in delete");
    }

    #[tokio::test]
    async fn test_create_project_missing_parameters() {
        init();
        let project_name = format!("{}-missing-params", get_test_project_name());

        // Clean up
        cleanup_project!(&TEST_CLIENT, &project_name);

        // Test: Missing description parameter
        let result = TEST_CLIENT
            .create_project(&project_name)
            // .description("test") - intentionally not set
            .send()
            .await;

        assert_missing_param!(result, "description");
        println!("✓ Correctly detected missing description parameter");
    }
}
