use super::*;
use crate::credentials::{CredentialsCache, SharedCredentialsProvider, DEFAULT_FETCH_TIMEOUT};
use serde_json::{json, Value};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    task::JoinHandle,
    time::timeout,
};

fn metadata() -> Value {
    json!({
        "Code": "Success",
        "AccessKeyId": "test-access-key-id",
        "AccessKeySecret": "test-access-key-secret",
        "SecurityToken": "test-security-token",
        "Expiration": "2099-01-02T03:04:05Z",
        "LastUpdated": "2026-09-09T02:00:00Z"
    })
}

fn parse(value: &Value) -> Result<Credentials, MetadataError> {
    parse_credentials(&serde_json::to_vec(value).unwrap())
}

#[test]
fn response_maps_all_fields_and_accepts_case_insensitive_success() {
    for code in ["Success", "SUCCESS", "success", "sUcCeSs"] {
        let mut value = metadata();
        value["Code"] = json!(code);
        value["Expiration"] = json!("2099-01-02T11:04:05+08:00");
        let credentials = parse(&value).unwrap();
        assert_eq!(credentials.access_key_id(), "test-access-key-id");
        assert_eq!(credentials.access_key_secret(), "test-access-key-secret");
        assert_eq!(credentials.security_token(), Some("test-security-token"));
        let expiration = DateTime::parse_from_rfc3339("2099-01-02T03:04:05Z").unwrap();
        let updated = DateTime::parse_from_rfc3339("2026-09-09T02:00:00Z").unwrap();
        assert_eq!(credentials.expiration(), Some(expiration.into()));
        assert_eq!(credentials.update_time(), Some(updated.into()));
    }
}

#[test]
fn validates_required_fields_and_timestamp_strings() {
    for field in [
        "Code",
        "AccessKeyId",
        "AccessKeySecret",
        "Expiration",
        "LastUpdated",
    ] {
        let mut missing = metadata();
        missing.as_object_mut().unwrap().remove(field);
        assert!(parse(&missing).is_err(), "missing {field}");
        for invalid in [Value::Null, json!(""), json!(42)] {
            let mut value = metadata();
            value[field] = invalid;
            assert!(parse(&value).is_err(), "invalid {field}");
        }
    }
    for field in ["Expiration", "LastUpdated"] {
        for invalid in ["not-a-time", "2026-02-30T00:00:00Z", "2026-09-09"] {
            let mut value = metadata();
            value[field] = json!(invalid);
            assert!(parse(&value).is_err(), "invalid {field}: {invalid}");
        }
    }
    for code in ["Failed", " Success", "Success "] {
        let mut value = metadata();
        value["Code"] = json!(code);
        assert!(parse(&value).is_err());
    }
    let mut epoch_updated = metadata();
    epoch_updated["LastUpdated"] = json!("1970-01-01T00:00:00Z");
    assert_eq!(
        parse(&epoch_updated).unwrap().update_time(),
        Some(std::time::UNIX_EPOCH)
    );
}

#[test]
fn security_token_is_optional_and_errors_do_not_disclose_response_values() {
    let mut missing = metadata();
    missing.as_object_mut().unwrap().remove("SecurityToken");
    assert_eq!(parse(&missing).unwrap().security_token(), None);
    for absent in [Value::Null, json!("")] {
        let mut value = metadata();
        value["SecurityToken"] = absent;
        assert_eq!(parse(&value).unwrap().security_token(), None);
    }
    let mut invalid = metadata();
    invalid["Expiration"] = json!("test-access-key-secret");
    for body in [
        serde_json::to_vec(&invalid).unwrap(),
        b"invalid JSON containing test-access-key-secret".to_vec(),
    ] {
        let error = parse_credentials(&body).unwrap_err();
        assert!(!format!("{error} {error:?}").contains("test-access-key-secret"));
    }
}

#[test]
fn constructor_only_requires_a_nonempty_role_name() {
    assert!(ecs_ram_role_credentials_provider("").is_err());
    for role in [
        ".",
        "..",
        " ",
        "role/name",
        "role\\name",
        "role?query",
        "role#fragment",
        "role%2Fname",
        "role\n",
    ] {
        assert!(
            ecs_ram_role_credentials_provider(role).is_ok(),
            "rejected {role:?}"
        );
    }
    let provider = ecs_ram_role_credentials_provider("my.ECS-role_123").unwrap();
    assert_eq!(
        provider.credentials_url.as_str(),
        format!("{METADATA_ENDPOINT}my.ECS-role_123")
    );
}

fn http_response(status: &str, body: &str, extra_headers: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n{body}",
        body.len()
    )
}

async fn mock_metadata(responses: Vec<String>) -> (url::Url, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = url::Url::parse(&format!(
        "http://{}/latest/meta-data/ram/security-credentials/",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let server = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (stream, _) = listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut request = String::new();
            reader.read_line(&mut request).await.unwrap();
            requests.push(request.trim_end().to_string());
            loop {
                let mut line = String::new();
                assert!(reader.read_line(&mut line).await.unwrap() > 0);
                if line == "\r\n" {
                    break;
                }
            }
            reader
                .get_mut()
                .write_all(response.as_bytes())
                .await
                .unwrap();
        }
        requests
    });
    (endpoint, server)
}

#[tokio::test]
async fn fetch_requests_only_the_specified_role_and_clones_work_concurrently() {
    let response = http_response("200 OK", &metadata().to_string(), "");
    let (endpoint, server) = mock_metadata(vec![response.clone(), response]).await;
    let provider =
        EcsRamRoleCredentialsProvider::with_endpoint("test-role".into(), endpoint).unwrap();
    let clone = provider.clone();
    let (first, second) = timeout(Duration::from_secs(5), async {
        tokio::join!(provider.fetch_credentials(), clone.fetch_credentials())
    })
    .await
    .unwrap();
    assert_eq!(first.unwrap().access_key_id(), "test-access-key-id");
    assert_eq!(
        second.unwrap().security_token(),
        Some("test-security-token")
    );
    assert_eq!(
        server.await.unwrap(),
        vec!["GET /latest/meta-data/ram/security-credentials/test-role HTTP/1.1"; 2]
    );
}

#[tokio::test]
async fn cache_retries_http_and_invalid_metadata_failures_then_caches_success() {
    let (endpoint, server) = mock_metadata(vec![
        http_response("503 Service Unavailable", "unavailable", ""),
        http_response("200 OK", r#"{"Code":"Failed"}"#, ""),
        http_response("200 OK", &metadata().to_string(), ""),
    ])
    .await;
    let provider =
        EcsRamRoleCredentialsProvider::with_endpoint("test-role".into(), endpoint).unwrap();
    let cache = CredentialsCache::new(
        SharedCredentialsProvider::new(provider),
        DEFAULT_FETCH_TIMEOUT,
    );
    let first = timeout(Duration::from_secs(5), cache.get())
        .await
        .unwrap()
        .unwrap();
    let requests = server.await.unwrap();
    assert_eq!(requests.len(), 3);
    // The server is now gone; the cached credentials still serve subsequent calls.
    assert!(std::sync::Arc::ptr_eq(&first, &cache.get().await.unwrap()));
    assert_eq!(first.access_key_secret(), "test-access-key-secret");
}

#[tokio::test]
async fn http_errors_and_redirects_are_rejected_without_following_location() {
    let target = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let location = format!(
        "Location: http://{}/elsewhere\r\n",
        target.local_addr().unwrap()
    );
    for status in ["401 Unauthorized", "404 Not Found", "302 Found"] {
        let (endpoint, server) = mock_metadata(vec![http_response(
            status,
            "test-access-key-secret",
            &location,
        )])
        .await;
        let provider =
            EcsRamRoleCredentialsProvider::with_endpoint("test-role".into(), endpoint).unwrap();
        let error = timeout(Duration::from_secs(5), provider.fetch_credentials())
            .await
            .unwrap()
            .unwrap_err();
        assert!(error.to_string().contains(&status[..3]));
        assert!(!format!("{error} {error:?}").contains("test-access-key-secret"));
        assert_eq!(server.await.unwrap().len(), 1);
    }
    assert!(timeout(Duration::from_millis(20), target.accept())
        .await
        .is_err());
}

#[tokio::test]
async fn exhausted_invalid_metadata_fetches_enter_the_shared_cooldown() {
    let mut invalid = metadata();
    invalid["LastUpdated"] = json!("invalid-timestamp");
    let response = http_response("200 OK", &invalid.to_string(), "");
    let (endpoint, server) = mock_metadata(vec![response; 3]).await;
    let provider =
        EcsRamRoleCredentialsProvider::with_endpoint("test-role".into(), endpoint).unwrap();
    let cache = CredentialsCache::new(
        SharedCredentialsProvider::new(provider),
        DEFAULT_FETCH_TIMEOUT,
    );
    assert!(matches!(
        timeout(Duration::from_secs(5), cache.get()).await.unwrap(),
        Err(CredentialsError::Provider(_))
    ));
    assert_eq!(server.await.unwrap().len(), 3);
    assert!(matches!(
        cache.get().await,
        Err(CredentialsError::Throttled { .. })
    ));
}
