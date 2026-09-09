use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::{
    sync::{Barrier, Semaphore},
    time::advance,
};

struct SequenceProvider {
    calls: AtomicUsize,
    results: Vec<Result<Credentials, CredentialsError>>,
}

#[async_trait]
impl CredentialsProvider for SequenceProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        self.results[call.min(self.results.len() - 1)].clone()
    }
}

fn sequence(results: Vec<Result<Credentials, CredentialsError>>) -> Arc<SequenceProvider> {
    Arc::new(SequenceProvider {
        calls: AtomicUsize::new(0),
        results,
    })
}

fn cache(provider: impl CredentialsProvider) -> CredentialsCache {
    CredentialsCache::new(
        SharedCredentialsProvider::new(provider),
        DEFAULT_FETCH_TIMEOUT,
    )
}

fn keys(id: &str) -> Credentials {
    Credentials::new(id, "secret").unwrap()
}
fn failure() -> Result<Credentials, CredentialsError> {
    Err(CredentialsError::provider(anyhow::anyhow!(
        "provider unavailable"
    )))
}

#[tokio::test(start_paused = true)]
async fn static_provider_preserves_metadata_and_obeys_cache_expiration_rules() {
    let expiration = SystemTime::now() + Duration::from_secs(3600);
    let update_time = SystemTime::now();
    let provider = StaticCredentialsProvider::new(
        keys("static-id")
            .with_security_token("static-token")
            .with_expiration(expiration)
            .with_update_time(update_time),
    );
    let clone = provider.clone();
    let (first, second) = tokio::join!(provider.fetch_credentials(), clone.fetch_credentials());
    for credentials in [first.unwrap(), second.unwrap()] {
        assert_eq!(credentials.access_key_id(), "static-id");
        assert_eq!(credentials.access_key_secret(), "secret");
        assert_eq!(credentials.security_token(), Some("static-token"));
        assert_eq!(credentials.expiration(), Some(expiration));
        assert_eq!(credentials.update_time(), Some(update_time));
    }
    let cached = cache(provider).get().await.unwrap();
    assert_eq!(cached.expiration(), Some(expiration));
    assert_eq!(cached.update_time(), Some(update_time));
    let expired = cache(StaticCredentialsProvider::new(
        keys("expired").with_expiration(SystemTime::UNIX_EPOCH),
    ));
    assert!(matches!(
        expired.get().await,
        Err(CredentialsError::Expired)
    ));
}

#[test]
fn static_helper_rejects_invalid_keys_before_client_construction() {
    assert!(matches!(
        static_credentials_provider("", "secret", None),
        Err(CredentialsError::InvalidAccessKey)
    ));
    assert!(matches!(
        static_credentials_provider("id", "", Some("token".into())),
        Err(CredentialsError::InvalidAccessKey)
    ));
}

#[tokio::test(start_paused = true)]
async fn legacy_static_config_and_explicit_static_provider_have_equivalent_credentials() {
    use crate::Config;
    for token in [None, Some(String::new()), Some("token".to_string())] {
        let explicit = Config::builder()
            .endpoint("localhost")
            .credentials_provider(
                static_credentials_provider("id", "secret", token.clone()).unwrap(),
            )
            .build()
            .unwrap();
        let legacy = Config::builder().endpoint("localhost");
        let legacy = match token {
            Some(token) => legacy.sts("id", "secret", token),
            None => legacy.access_key("id", "secret"),
        }
        .build()
        .unwrap();
        let credentials = explicit.credentials.get().await.unwrap();
        let old_api_credentials = legacy.credentials.get().await.unwrap();
        assert_eq!(
            credentials.access_key_id(),
            old_api_credentials.access_key_id()
        );
        assert_eq!(
            credentials.access_key_secret(),
            old_api_credentials.access_key_secret()
        );
        assert_eq!(
            credentials.security_token(),
            old_api_credentials.security_token()
        );
        assert_eq!(credentials.expiration(), None);
        assert_eq!(credentials.update_time(), None);
        advance(Duration::from_secs(86400 * 365)).await;
        assert!(Arc::ptr_eq(
            &credentials,
            &explicit.credentials.get().await.unwrap()
        ));
        assert!(Arc::ptr_eq(
            &old_api_credentials,
            &legacy.credentials.get().await.unwrap()
        ));
    }
}

#[test]
fn validates_keys_and_preserves_metadata_without_exposing_secrets() {
    assert!(matches!(
        Credentials::new("", "secret"),
        Err(CredentialsError::InvalidAccessKey)
    ));
    assert!(matches!(
        Credentials::new("id", ""),
        Err(CredentialsError::InvalidAccessKey)
    ));
    let expiration = SystemTime::now() + Duration::from_secs(3600);
    let update_time = SystemTime::now();
    let credentials = keys("private-id")
        .with_security_token("private-token")
        .with_expiration(expiration)
        .with_update_time(update_time);
    assert_eq!(credentials.access_key_id(), "private-id");
    assert_eq!(credentials.access_key_secret(), "secret");
    assert_eq!(credentials.security_token(), Some("private-token"));
    assert_eq!(credentials.expiration(), Some(expiration));
    assert_eq!(credentials.update_time(), Some(update_time));
    let debug = format!("{credentials:?}");
    assert!(!debug.contains("private-id"));
    assert!(!debug.contains("private-token"));
    assert!(!debug.contains("\"secret\""));
    assert_eq!(keys("id").with_security_token("").security_token(), None);
}

#[tokio::test(start_paused = true)]
async fn nonexpiring_credentials_are_fetched_once() {
    let provider = sequence(vec![Ok(keys("first")), Ok(keys("second"))]);
    let cache = cache(provider.clone());
    let first = cache.get().await.unwrap();
    advance(Duration::from_secs(86400 * 365)).await;
    let second = cache.get().await.unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test(start_paused = true)]
async fn refresh_deadline_is_sampled_once_and_refreshes_before_expiration() {
    for ttl in [60, 3600] {
        let provider = sequence(vec![
            Ok(keys("first").with_expiration(SystemTime::now() + Duration::from_secs(ttl))),
            Ok(keys("second")),
        ]);
        let cache = cache(provider.clone());
        assert_eq!(cache.get().await.unwrap().access_key_id(), "first");
        let entry = cache.current.load_full().unwrap();
        let delay = entry.refresh_after.unwrap();
        let window = (ttl / 5).min(300);
        assert!(delay >= Duration::from_secs(ttl - window - 1));
        assert!(delay <= Duration::from_secs(ttl - window / 2));
        advance(delay - Duration::from_millis(1)).await;
        assert_eq!(cache.get().await.unwrap().access_key_id(), "first");
        assert_eq!(
            cache.current.load_full().unwrap().refresh_after,
            Some(delay)
        );
        advance(Duration::from_millis(1)).await;
        assert_eq!(cache.get().await.unwrap().access_key_id(), "second");
        assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    }
}

#[tokio::test(start_paused = true)]
async fn successful_third_attempt_does_not_start_cooldown() {
    let provider = sequence(vec![failure(), failure(), Ok(keys("third"))]);
    let cache = cache(provider.clone());
    let start = Instant::now();
    assert_eq!(cache.get().await.unwrap().access_key_id(), "third");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    assert!(start.elapsed() >= Duration::from_millis(300));
    assert!(cache.failure.load().is_none());
}

#[tokio::test(start_paused = true)]
async fn cooldown_starts_after_exhaustion_and_is_not_extended_by_reads() {
    let provider = sequence(vec![failure(), failure(), failure(), Ok(keys("recovered"))]);
    let cache = cache(provider.clone());
    assert!(matches!(
        cache.get().await,
        Err(CredentialsError::Provider(_))
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    for _ in 0..14 {
        advance(Duration::from_secs(1)).await;
        match cache.get().await.unwrap_err() {
            CredentialsError::Throttled { source, .. } => {
                assert!(matches!(*source, CredentialsError::Provider(_)));
            }
            error => panic!("unexpected error: {error}"),
        }
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    advance(Duration::from_secs(1)).await;
    assert_eq!(cache.get().await.unwrap().access_key_id(), "recovered");
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
}

#[tokio::test(start_paused = true)]
async fn expired_cached_credentials_survive_failure_and_throttling() {
    let provider = sequence(vec![failure()]);
    let cache = cache(provider.clone());
    // Simulate credentials previously fetched successfully that have since expired.
    let old = Arc::new(keys("old").with_expiration(SystemTime::UNIX_EPOCH));
    cache.current.store(Some(Arc::new(CachedCredentials {
        credentials: old.clone(),
        fetched_at: Instant::now(),
        refresh_after: Some(Duration::ZERO),
    })));
    assert!(Arc::ptr_eq(&cache.get().await.unwrap(), &old));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    assert!(Arc::ptr_eq(&cache.get().await.unwrap(), &old));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    advance(FAILURE_COOLDOWN).await;
    assert!(Arc::ptr_eq(&cache.get().await.unwrap(), &old));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 6);
}

#[tokio::test(start_paused = true)]
async fn newly_fetched_expired_credentials_are_retried_and_rejected() {
    let provider = sequence(vec![Ok(
        keys("expired").with_expiration(SystemTime::UNIX_EPOCH)
    )]);
    let cache = cache(provider.clone());
    assert!(matches!(cache.get().await, Err(CredentialsError::Expired)));
    assert!(cache.current.load().is_none());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
}

struct PendingProvider {
    calls: AtomicUsize,
    dropped: AtomicUsize,
}
#[async_trait]
impl CredentialsProvider for PendingProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        struct OnDrop<'a>(&'a AtomicUsize);
        impl Drop for OnDrop<'_> {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        let _guard = OnDrop(&self.dropped);
        std::future::pending().await
    }
}

#[tokio::test(start_paused = true)]
async fn timeout_cancels_each_attempt_then_starts_cooldown() {
    let provider = Arc::new(PendingProvider {
        calls: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    let cache = CredentialsCache::new(
        SharedCredentialsProvider::new(provider.clone()),
        Duration::from_secs(2),
    );
    let start = Instant::now();
    assert!(
        matches!(cache.get().await, Err(CredentialsError::Timeout(duration)) if duration == Duration::from_secs(2))
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
    assert_eq!(provider.dropped.load(Ordering::SeqCst), 3);
    assert!(start.elapsed() >= Duration::from_millis(6300));
    assert!(start.elapsed() < Duration::from_millis(6400));
    assert!(matches!(
        cache.get().await,
        Err(CredentialsError::Throttled { .. })
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 3);
}

#[tokio::test(start_paused = true)]
async fn cancelling_request_drops_fetch_without_poisoning_future_fetches() {
    let provider = Arc::new(PendingProvider {
        calls: AtomicUsize::new(0),
        dropped: AtomicUsize::new(0),
    });
    let cache = cache(provider.clone());
    for _ in 0..2 {
        assert!(timeout(Duration::from_millis(10), cache.get())
            .await
            .is_err());
        assert!(cache.failure.load().is_none());
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    assert_eq!(provider.dropped.load(Ordering::SeqCst), 2);
}

struct ConcurrentProvider {
    calls: AtomicUsize,
    barrier: Barrier,
}
#[async_trait]
impl CredentialsProvider for ConcurrentProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.barrier.wait().await;
        failure()
    }
}

#[tokio::test(start_paused = true)]
async fn concurrent_rounds_each_get_three_attempts_without_single_flight() {
    let provider = Arc::new(ConcurrentProvider {
        calls: AtomicUsize::new(0),
        barrier: Barrier::new(4),
    });
    let cache = Arc::new(cache(provider.clone()));
    let mut tasks = Vec::new();
    for _ in 0..4 {
        let cache = cache.clone();
        tasks.push(tokio::spawn(async move { cache.get().await }));
    }
    for task in tasks {
        assert!(matches!(
            task.await.unwrap(),
            Err(CredentialsError::Provider(_))
        ));
    }
    assert_eq!(provider.calls.load(Ordering::SeqCst), 12);
    assert!(matches!(
        cache.get().await,
        Err(CredentialsError::Throttled { .. })
    ));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 12);
}

struct RacingProvider {
    calls: AtomicUsize,
    release: Semaphore,
}
#[async_trait]
impl CredentialsProvider for RacingProvider {
    async fn fetch_credentials(&self) -> Result<Credentials, CredentialsError> {
        match self.calls.fetch_add(1, Ordering::SeqCst) {
            0 => {
                self.release.acquire().await.unwrap().forget();
                failure()
            }
            1 => Ok(keys("concurrent-success")),
            _ => failure(),
        }
    }
}

#[tokio::test(start_paused = true)]
async fn concurrent_failure_preserves_and_returns_latest_success() {
    let provider = Arc::new(RacingProvider {
        calls: AtomicUsize::new(0),
        release: Semaphore::new(0),
    });
    let cache = Arc::new(cache(provider.clone()));
    let first = tokio::spawn({
        let cache = cache.clone();
        async move { cache.get().await }
    });
    tokio::task::yield_now().await;
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let success = cache.get().await.unwrap();
    provider.release.add_permits(1);
    assert!(Arc::ptr_eq(&first.await.unwrap().unwrap(), &success));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 4);
    assert!(Arc::ptr_eq(&cache.get().await.unwrap(), &success));
}

#[test]
fn config_build_is_lazy_and_rejects_conflicting_sources_and_zero_timeout() {
    use crate::{Client, Config, FromConfig};
    let provider = sequence(vec![Ok(keys("id"))]);
    let config = Config::builder()
        .endpoint("localhost")
        .credentials_provider(provider.clone())
        .build()
        .unwrap();
    Client::from_config(config).unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert!(Config::builder()
        .endpoint("localhost")
        .credentials_provider(provider.clone())
        .access_key("id", "secret")
        .build()
        .is_err());
    assert!(Config::builder()
        .endpoint("localhost")
        .sts("id", "secret", "token")
        .credentials_provider(provider.clone())
        .build()
        .is_err());
    assert!(Config::builder()
        .endpoint("localhost")
        .credentials_provider(provider)
        .credentials_fetch_timeout(Duration::ZERO)
        .build()
        .is_err());
    assert!(Config::builder().endpoint("localhost").build().is_err());
    assert!(Config::builder()
        .endpoint("localhost")
        .access_key("", "secret")
        .build()
        .is_err());
}

#[tokio::test(start_paused = true)]
async fn config_clones_share_cache_while_independent_configs_do_not() {
    use crate::Config;
    let provider = sequence(vec![Ok(keys("id"))]);
    let shared = SharedCredentialsProvider::new(provider.clone());
    let config = Config::builder()
        .endpoint("localhost")
        .credentials_provider(shared.clone())
        .build()
        .unwrap();
    let clone = config.clone();
    assert!(Arc::ptr_eq(&config.credentials, &clone.credentials));
    let first = config.credentials.get().await.unwrap();
    assert!(Arc::ptr_eq(&first, &clone.credentials.get().await.unwrap()));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
    let other = Config::builder()
        .endpoint("localhost")
        .credentials_provider(shared)
        .build()
        .unwrap();
    other.credentials.get().await.unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    let fixed = Config::builder()
        .endpoint("localhost")
        .sts("id", "secret", "token")
        .build()
        .unwrap();
    assert_eq!(
        fixed.credentials.get().await.unwrap().security_token(),
        Some("token")
    );
}
