use aliyun_log_producer::{
    BaseProducer as RustProducer, Log as RustLog, LogstoreWriter as RustWriter, SendOptions,
};
use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};

use crate::{
    arguments::Arguments,
    callback::Callback,
    config::{duration, ProducerConfig},
    credentials::ExternalCredentials,
    error::{producer_error, ConfigError, InvalidArgumentError},
};

/// An immutable snapshot of a log, preserving content order and duplicate keys.
#[pyclass(frozen, module = "aliyun_log_producer._native")]
pub(crate) struct Log {
    inner: RustLog,
}

#[pymethods]
impl Log {
    #[new]
    #[pyo3(signature = (*args, **kwargs), text_signature = "(contents, *, time=None, time_ns=None)")]
    fn new(args: &Bound<'_, PyTuple>, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        let args = Arguments::new(
            args,
            kwargs,
            &["contents", "time", "time_ns"],
            1,
            InvalidArgumentError::new_err,
        )?;
        Self::from_values(
            args.required("contents")?,
            args.optional("time")?,
            args.optional("time_ns")?,
        )
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

impl Log {
    fn from_values(
        contents: Vec<(String, String)>,
        time: Option<u32>,
        time_ns: Option<u32>,
    ) -> PyResult<Self> {
        if time_ns.is_some_and(|value| value >= 1_000_000_000) {
            return Err(InvalidArgumentError::new_err(
                "time_ns must be in [0, 999999999]",
            ));
        }
        let mut inner = time
            .map(RustLog::from_unixtime)
            .unwrap_or_else(|| aliyun_log_producer::log!());
        if let Some(ns) = time_ns {
            inner.set_time_ns(ns);
        }
        for (key, value) in contents {
            inner.add_content_kv(key, value);
        }
        Ok(Self { inner })
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
            return Err(ConfigError::new_err(
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
    #[pyo3(signature = (*args, **kwargs), text_signature = "($self, log, *, time=None, time_ns=None, source='', topic='', on_delivery=None)")]
    fn send(
        &self,
        py: Python<'_>,
        args: &Bound<'_, PyTuple>,
        kwargs: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let args = Arguments::new(
            args,
            kwargs,
            &["log", "time", "time_ns", "source", "topic", "on_delivery"],
            1,
            InvalidArgumentError::new_err,
        )?;
        let log: Bound<'_, PyAny> = args.required("log")?;
        let time: Option<u32> = args.optional("time")?;
        let time_ns: Option<u32> = args.optional("time_ns")?;
        let source = args.default("source", String::new())?;
        let topic = args.default("topic", String::new())?;
        let on_delivery: Option<Bound<'_, PyAny>> = args.optional("on_delivery")?;
        let log = if let Ok(existing) = log.cast::<Log>() {
            if time.is_some() || time_ns.is_some() {
                return Err(InvalidArgumentError::new_err(
                    "time and time_ns are only supported for dict input; set them when creating Log",
                ));
            }
            existing.borrow().inner.clone()
        } else if let Ok(contents) = log.cast::<PyDict>() {
            let contents = contents
                .iter()
                .map(|(key, value)| {
                    Ok((
                        key.extract::<String>().map_err(|_| {
                            InvalidArgumentError::new_err("log keys must be strings")
                        })?,
                        value.extract::<String>().map_err(|_| {
                            InvalidArgumentError::new_err("log values must be strings")
                        })?,
                    ))
                })
                .collect::<PyResult<Vec<_>>>()?;
            // Construct owned Rust data directly: no temporary Python Log object
            // or second clone before enqueueing. Preserve dictionary order.
            Log::from_values(contents, time, time_ns)?.inner
        } else {
            return Err(InvalidArgumentError::new_err(
                "log must be a Log or dict[str, str]",
            ));
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
