use super::*;
use std::{collections::HashMap, process::Command};

fn read(values: &[(&str, &str)]) -> Result<Credentials, CredentialsError> {
    let values: HashMap<_, _> = values.iter().copied().collect();
    read_credentials("ID", "SECRET", "TOKEN", |name| {
        values
            .get(name)
            .map(|value| value.to_string())
            .ok_or(VarError::NotPresent)
    })
}

#[test]
fn access_keys_must_exist_and_be_nonempty() {
    for values in [vec![], vec![("ID", "id")], vec![("SECRET", "secret")]] {
        assert!(matches!(read(&values), Err(CredentialsError::Provider(_))));
    }
    for (id, secret) in [("", "secret"), ("id", ""), ("", "")] {
        assert!(matches!(
            read(&[("ID", id), ("SECRET", secret)]),
            Err(CredentialsError::InvalidAccessKey)
        ));
    }
}

#[test]
fn missing_or_empty_token_is_valid_and_values_are_not_trimmed() {
    for token in [None, Some(""), Some(" token ")] {
        let mut values = vec![("ID", " id "), ("SECRET", " secret ")];
        if let Some(token) = token {
            values.push(("TOKEN", token));
        }
        let credentials = read(&values).unwrap();
        assert_eq!(credentials.access_key_id(), " id ");
        assert_eq!(credentials.access_key_secret(), " secret ");
        assert_eq!(
            credentials.security_token(),
            token.filter(|s| !s.is_empty())
        );
        assert_eq!(credentials.expiration(), None);
        assert_eq!(credentials.update_time(), None);
    }
}

#[test]
fn non_unicode_errors_do_not_disclose_values() {
    for invalid in ["ID", "SECRET", "TOKEN"] {
        let error = read_credentials("ID", "SECRET", "TOKEN", |name| {
            if name == invalid {
                Err(VarError::NotUnicode("sensitive-value".into()))
            } else {
                Ok("valid-value".into())
            }
        })
        .unwrap_err();
        assert!(matches!(error, CredentialsError::Provider(_)));
        let display = format!("{error} {error:?}");
        assert!(display.contains(invalid));
        assert!(!display.contains("sensitive-value"));
        assert!(!display.contains("valid-value"));
    }
}

#[test]
fn helpers_read_environment_at_construction() {
    // Give child processes isolated environments without mutating the test runner's
    // process-wide environment while other tests are running.
    if std::env::var_os("ENV_PROVIDER_TEST_CHILD").is_some() {
        let provider = environment_credentials_provider().unwrap();
        let custom = environment_credentials_provider_builder()
            .with_access_key_id_env("CUSTOM_ID")
            .with_access_key_secret_env("CUSTOM_SECRET")
            .with_security_token_env("CUSTOM_TOKEN")
            .build()
            .unwrap();
        let credentials = tokio_test::block_on(provider.fetch_credentials()).unwrap();
        assert_eq!(credentials.access_key_id(), "default-id");
        assert_eq!(credentials.access_key_secret(), "default-secret");
        assert_eq!(
            credentials.security_token(),
            std::env::var("ALIBABA_CLOUD_SECURITY_TOKEN")
                .ok()
                .filter(|s| !s.is_empty())
                .as_deref()
        );
        let cloned = tokio_test::block_on(custom.clone().fetch_credentials()).unwrap();
        assert_eq!(cloned.access_key_id(), "custom-id");
        assert_eq!(cloned.access_key_secret(), "custom-secret");
        assert_eq!(cloned.security_token(), Some("custom-token"));
        for (id_name, secret_name, token_name) in [
            (Some("CUSTOM_ID"), None, None),
            (None, Some("CUSTOM_SECRET"), None),
            (None, None, Some("CUSTOM_TOKEN")),
        ] {
            let mut builder = environment_credentials_provider_builder();
            if let Some(name) = id_name {
                builder = builder.with_access_key_id_env(name);
            }
            if let Some(name) = secret_name {
                builder = builder.with_access_key_secret_env(name);
            }
            if let Some(name) = token_name {
                builder = builder.with_security_token_env(name);
            }
            let partial = builder.build().unwrap();
            let result = tokio_test::block_on(partial.fetch_credentials()).unwrap();
            assert_eq!(
                result.access_key_id(),
                if id_name.is_some() {
                    "custom-id"
                } else {
                    "default-id"
                }
            );
            assert_eq!(
                result.access_key_secret(),
                if secret_name.is_some() {
                    "custom-secret"
                } else {
                    "default-secret"
                }
            );
            assert_eq!(
                result.security_token(),
                if token_name.is_some() {
                    Some("custom-token")
                } else {
                    credentials.security_token()
                }
            );
        }
        assert_eq!(cloned.expiration(), None);
        assert_eq!(cloned.update_time(), None);
        assert!(environment_credentials_provider_builder()
            .with_access_key_id_env("MISSING_ID")
            .build()
            .is_err());
        assert!(matches!(
            environment_credentials_provider_builder()
                .with_access_key_id_env("EMPTY_ID")
                .build(),
            Err(CredentialsError::InvalidAccessKey)
        ));
        assert!(!format!("{custom:?}").contains("custom-secret"));
        return;
    }
    for token in [None, Some(""), Some("default-token")] {
        let mut child = Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "credentials::environment::tests::helpers_read_environment_at_construction",
            ])
            .env_clear()
            .env("ENV_PROVIDER_TEST_CHILD", "1")
            .env("ALIBABA_CLOUD_ACCESS_KEY_ID", "default-id")
            .env("ALIBABA_CLOUD_ACCESS_KEY_SECRET", "default-secret")
            .env("CUSTOM_ID", "custom-id")
            .env("CUSTOM_SECRET", "custom-secret")
            .env("CUSTOM_TOKEN", "custom-token")
            .env("EMPTY_ID", "");
        if let Some(token) = token {
            child.env("ALIBABA_CLOUD_SECURITY_TOKEN", token);
        }
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
