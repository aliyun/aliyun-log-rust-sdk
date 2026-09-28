use std::{
    fmt, io,
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};

use aliyun_log_rust_sdk::{
    async_trait, Credentials as RustCredentials, CredentialsError,
    CredentialsProvider as RustCredentialsProvider,
};
use pyo3::{exceptions::PyValueError, prelude::*};
use tokio::sync::oneshot;

/// One immutable, internally consistent credentials snapshot.
#[pyclass(frozen, module = "aliyun_log_producer._native")]
pub(crate) struct Credentials {
    inner: RustCredentials,
}

#[pymethods]
impl Credentials {
    #[new]
    #[pyo3(signature = (*, access_key_id, access_key_secret, security_token=None, expires_at=None))]
    fn new(
        access_key_id: String,
        access_key_secret: String,
        security_token: Option<String>,
        expires_at: Option<u64>,
    ) -> PyResult<Self> {
        let mut inner = RustCredentials::new(access_key_id, access_key_secret)
            .map_err(|e| PyValueError::new_err(e.to_string()))?;
        if let Some(token) = security_token {
            inner = inner.with_security_token(token);
        }
        if let Some(expiration) = expires_at {
            let expiration = SystemTime::UNIX_EPOCH
                .checked_add(Duration::from_secs(expiration))
                .ok_or_else(|| {
                    PyValueError::new_err("expires_at is outside the supported timestamp range")
                })?;
            inner = inner.with_expiration(expiration);
        }
        Ok(Self { inner })
    }

    #[getter]
    fn access_key_id(&self) -> &str {
        self.inner.access_key_id()
    }
    #[getter]
    fn access_key_secret(&self) -> &str {
        self.inner.access_key_secret()
    }
    #[getter]
    fn security_token(&self) -> Option<&str> {
        self.inner.security_token()
    }
    #[getter]
    fn expires_at(&self) -> Option<u64> {
        self.inner.expiration().map(|time| {
            time.duration_since(SystemTime::UNIX_EPOCH)
                .expect("validated timestamp")
                .as_secs()
        })
    }
    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

type Outcome = Result<RustCredentials, CredentialsError>;

/// Forward each fetch to the Python provider on an independent worker thread.
/// Credential caching and refresh policy belong to the Rust client.
pub(crate) struct PythonCredentialsProvider {
    provider: Arc<Py<PyAny>>,
    start_python_fetch: Option<Arc<Py<PyAny>>>,
}

impl fmt::Debug for PythonCredentialsProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Do not invoke user-defined __repr__ or expose provider contents.
        f.write_str("PythonCredentialsProvider(..)")
    }
}

fn failed(message: impl Into<String>) -> CredentialsError {
    CredentialsError::provider(io::Error::other(message.into()))
}

impl PythonCredentialsProvider {
    pub(crate) fn new(provider: Bound<'_, PyAny>) -> PyResult<Self> {
        let py = provider.py();
        let implementation: String = py
            .import("sys")?
            .getattr("implementation")?
            .getattr("name")?
            .extract()?;
        let start_python_fetch = if implementation == "graalpy" {
            Some(Arc::new(
                py.import("aliyun_log_producer.credentials")?
                    .getattr("_start_fetch")?
                    .unbind(),
            ))
        } else {
            None
        };
        Ok(Self {
            provider: Arc::new(provider.unbind()),
            start_python_fetch,
        })
    }
}

fn invoke_provider(provider: &Py<PyAny>) -> Outcome {
    Python::try_attach(|py| {
        let _fetch_guard = FetchGuard::enter();
        let value = provider
            .bind(py)
            .call_method0("get_credentials")
            .map_err(|error| {
                let kind = error
                    .get_type(py)
                    .name()
                    .map(|name| name.to_string())
                    .unwrap_or_else(|_| "Exception".into());
                // Exception messages/tracebacks can contain credentials.
                failed(format!(
                    "Python get_credentials() raised {kind}; exception details omitted"
                ))
            })?;
        let snapshot = value
            .cast::<Credentials>()
            .map_err(|_| failed("Python get_credentials() must return Credentials"))?;
        Ok(snapshot.borrow().inner.clone())
    })
    .unwrap_or_else(|| {
        Err(failed(
            "Python interpreter unavailable for credentials fetch",
        ))
    })
}

// GraalPy must own the thread executing application Python. Leaving a native
// thread inside get_credentials() prevents its polyglot context from closing.
#[pyclass(
    name = "_CredentialsCompletion",
    frozen,
    module = "aliyun_log_producer._native"
)]
pub(crate) struct CredentialsCompletion {
    sender: Arc<Mutex<Option<oneshot::Sender<Outcome>>>>,
}

#[pymethods]
impl CredentialsCompletion {
    fn _enter(&self) {
        FETCHING.with(|flag| flag.set(true));
    }

    fn _exit(&self) {
        FETCHING.with(|flag| flag.set(false));
    }

    fn _succeeded(&self, credentials: &Credentials) {
        complete(&self.sender, Ok(credentials.inner.clone()));
    }

    fn _failed(&self, exception_type: Option<String>) {
        let message = exception_type.map_or_else(
            || "Python get_credentials() must return Credentials".to_owned(),
            |kind| format!("Python get_credentials() raised {kind}; exception details omitted"),
        );
        complete(&self.sender, Err(failed(message)));
    }
}

fn complete(sender: &Mutex<Option<oneshot::Sender<Outcome>>>, result: Outcome) {
    // Drop the lock before waking the Rust receiver or dropping its result.
    let sender = sender.lock().unwrap().take();
    if let Some(sender) = sender {
        let _ = sender.send(result);
    }
}

fn start_python_fetch(provider: &Py<PyAny>, start: &Py<PyAny>, sender: oneshot::Sender<Outcome>) {
    let sender = Arc::new(Mutex::new(Some(sender)));
    let result = Python::try_attach(|py| -> PyResult<()> {
        let completion = Py::new(
            py,
            CredentialsCompletion {
                sender: sender.clone(),
            },
        )?;
        start.bind(py).call1((provider.bind(py), completion))?;
        Ok(())
    });
    if !matches!(result, Some(Ok(()))) {
        // Never expose exception text from application code or startup hooks.
        complete(
            &sender,
            Err(failed("could not start Python credentials worker")),
        );
    }
}

#[async_trait]
impl RustCredentialsProvider for PythonCredentialsProvider {
    async fn fetch_credentials(&self) -> Outcome {
        let provider = self.provider.clone();
        let start = self.start_python_fetch.clone();
        let (sender, receiver) = oneshot::channel();
        // A synchronous Python call must not block the IO runtime. Dropping a
        // timed-out waiter leaves this invocation independent of later fetches.
        std::thread::Builder::new()
            .name("sls-python-credentials".into())
            .spawn(move || {
                if let Some(start) = start {
                    start_python_fetch(&provider, &start, sender);
                    return;
                }
                let result = catch_unwind(AssertUnwindSafe(|| invoke_provider(&provider)))
                    .unwrap_or_else(|_| Err(failed("Python credentials worker panicked")));
                let _ = sender.send(result);
            })
            .map_err(|_| failed("could not start Python credentials worker"))?;
        receiver
            .await
            .map_err(|_| failed("Python credentials worker exited without a result"))?
    }
}

thread_local! {
    static FETCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(crate) fn is_fetching() -> bool {
    FETCHING.with(|flag| flag.get())
}
struct FetchGuard;
impl FetchGuard {
    fn enter() -> Self {
        FETCHING.with(|flag| flag.set(true));
        Self
    }
}
impl Drop for FetchGuard {
    fn drop(&mut self) {
        FETCHING.with(|flag| flag.set(false));
    }
}
