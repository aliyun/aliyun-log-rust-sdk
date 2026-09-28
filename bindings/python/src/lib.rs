//! Python adaptation only; all delivery and scheduling belong to the Rust producer.
mod arguments;
mod callback;
mod config;
mod credentials;
mod error;
mod producer;

use pyo3::prelude::*;

#[pymodule(gil_used = true)]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<config::ProducerConfig>()?;
    m.add_class::<credentials::Credentials>()?;
    m.add_class::<credentials::ExternalCredentials>()?;
    m.add_class::<producer::Log>()?;
    m.add_class::<producer::NativeBaseProducer>()?;
    m.add_class::<producer::LogstoreWriter>()?;
    m.add_class::<error::DeliveryError>()?;
    error::register(m)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
