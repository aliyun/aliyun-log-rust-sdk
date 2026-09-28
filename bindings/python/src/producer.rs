use aliyun_log_producer::{
    BaseProducer as RustProducer, Log as RustLog, LogstoreWriter as RustWriter, SendOptions,
};
use pyo3::{
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::PyDict,
};

use crate::{
    callback::Callback,
    config::{duration, ProducerConfig},
    credentials::ExternalCredentials,
    error::producer_error,
};

/// An immutable snapshot of a log, preserving content order and duplicate keys.
#[pyclass(frozen, module = "aliyun_log_producer._native")]
pub(crate) struct Log {
    inner: RustLog,
}

#[pymethods]
impl Log {
    #[new]
    #[pyo3(signature = (contents, *, time=None, time_ns=None))]
    fn new(
        contents: Vec<(String, String)>,
        time: Option<u32>,
        time_ns: Option<u32>,
    ) -> PyResult<Self> {
        if time_ns.is_some_and(|value| value >= 1_000_000_000) {
            return Err(PyValueError::new_err("time_ns must be in [0, 999999999]"));
        }
        let mut inner = time
            .map(RustLog::from_unixtime)
            .unwrap_or_else(aliyun_log_producer::log_now);
        if let Some(ns) = time_ns {
            inner.set_time_ns(ns);
        }
        for (key, value) in contents {
            inner.add_content_kv(key, value);
        }
        Ok(Self { inner })
    }

    #[getter]
    fn time(&self) -> u32 {
        *self.inner.time()
    }

    #[getter]
    fn time_ns(&self) -> Option<u32> {
        *self.inner.time_ns()
    }

    #[getter]
    fn contents(&self) -> Vec<(String, String)> {
        self.inner
            .contents()
            .iter()
            .map(|item| (item.key().clone(), item.value().clone()))
            .collect()
    }
}

#[pyclass(name = "_BaseProducer", frozen, module = "aliyun_log_producer._native")]
pub(crate) struct NativeBaseProducer {
    inner: RustProducer,
}

#[pymethods]
impl NativeBaseProducer {
    #[new]
    #[pyo3(signature = (config, *, external_credentials))]
    fn new(
        py: Python<'_>,
        config: &ProducerConfig,
        external_credentials: Option<&ExternalCredentials>,
    ) -> PyResult<Self> {
        if config.credentials_provider.is_some() != external_credentials.is_some() {
            return Err(PyValueError::new_err(
                "external_credentials must be provided for a credentials_provider and must be None for static credentials",
            ));
        }
        let mut config = config.inner.clone();
        if let Some(credentials) = external_credentials {
            config = config.with_external_managed_credentials(credentials.inner.clone());
        }
        py.detach(move || RustProducer::create(config))
            .map(|inner| Self { inner })
            .map_err(producer_error)
    }

    #[pyo3(signature = (project, logstore, _owner=None))]
    fn _writer(
        &self,
        py: Python<'_>,
        project: String,
        logstore: String,
        _owner: Option<Py<PyAny>>,
    ) -> PyResult<LogstoreWriter> {
        py.detach(|| self.inner.writer(&project, &logstore))
            .map(|inner| LogstoreWriter { inner, _owner })
            .map_err(producer_error)
    }

    /// Wait for delivery of previously admitted logs; callbacks may still be running.
    fn _flush(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.flush_blocking())
            .map_err(producer_error)
    }

    fn _begin_close(&self) {
        self.inner.begin_close();
    }

    fn _is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    fn _wait_closed(&self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.wait_closed_blocking())
            .map_err(producer_error)
    }

    /// Wait for events, then dispatch their callbacks on the calling Python thread.
    fn _poll(&self, py: Python<'_>, timeout: f64) -> PyResult<usize> {
        let timeout = duration(timeout, "timeout")?;
        let batch = py
            .detach(|| self.inner.poll_batch(timeout))
            .map_err(producer_error)?;
        Ok(batch.dispatch())
    }
}

#[pyclass(frozen, module = "aliyun_log_producer._native")]
pub(crate) struct LogstoreWriter {
    inner: RustWriter,
    // Keep the Python controller alive as long as a writer is usable. The poll
    // thread itself never retains that controller.
    _owner: Option<Py<PyAny>>,
}

#[pymethods]
impl LogstoreWriter {
    /// Admit a Log or a snapshot of a string dictionary immediately.
    /// time/time_ns apply only to dictionary input. Admission failure raises
    /// without invoking on_delivery.
    #[pyo3(signature = (log, *, time=None, time_ns=None, source="", topic="", on_delivery=None))]
    #[allow(clippy::too_many_arguments)]
    fn send(
        &self,
        py: Python<'_>,
        log: &Bound<'_, PyAny>,
        time: Option<u32>,
        time_ns: Option<u32>,
        source: &str,
        topic: &str,
        on_delivery: Option<Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let log = if let Ok(existing) = log.cast::<Log>() {
            if time.is_some() || time_ns.is_some() {
                return Err(PyTypeError::new_err(
                    "time and time_ns are only supported for dict input; set them when creating Log",
                ));
            }
            existing.borrow().inner.clone()
        } else if let Ok(contents) = log.cast::<PyDict>() {
            let contents = contents
                .iter()
                .map(|(key, value)| Ok((key.extract::<String>()?, value.extract::<String>()?)))
                .collect::<PyResult<Vec<_>>>()?;
            // Construct owned Rust data directly: no temporary Python Log object
            // or second clone before enqueueing. Preserve dictionary order.
            Log::new(contents, time, time_ns)?.inner
        } else {
            return Err(PyTypeError::new_err("log must be a Log or dict[str, str]"));
        };
        let callback = on_delivery.map(Callback::new);
        let writer = &self.inner;
        let options = SendOptions::default().with_source(source).with_topic(topic);
        py.detach(move || match callback {
            None => writer.send_with_options(log, options),
            Some(callback) => writer
                .send_with_options_and_callback(log, options, move |result| callback.run(result)),
        })
        .map_err(producer_error)
    }
}
