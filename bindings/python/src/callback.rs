use aliyun_log_producer::DeliveryResult;
use pyo3::prelude::*;

use crate::error::DeliveryError;

// No Python references are stored in the core producer; this owned handle
// lives only in its FnOnce. The caller supplies a synchronous callable.
pub(crate) struct Callback {
    callable: Py<PyAny>,
}

impl Callback {
    pub(crate) fn new(callable: Bound<'_, PyAny>) -> Self {
        Self {
            callable: callable.unbind(),
        }
    }

    pub(crate) fn run(self, result: &DeliveryResult) {
        // Invoked only by the Python-owned poll thread with the GIL already held.
        // Nested attachment reuses that attachment for every callback in the batch.
        Python::try_attach(move |py| {
            let callable = self.callable.bind(py);
            let invoke = || -> PyResult<()> {
                let argument = match result {
                    Ok(()) => py.None(),
                    Err(error) => Py::new(py, DeliveryError::from(error))?.into_any(),
                };
                callable.call1((argument,))?;
                Ok(())
            };
            if let Err(error) = invoke() {
                error.write_unraisable(py, Some(callable));
            }
        });
    }
}
