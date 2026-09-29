//! Bind public arguments before conversion so PyO3 cannot leak built-in errors.
use pyo3::{
    prelude::*,
    types::{PyDict, PyTuple},
};

pub(crate) struct Arguments<'py> {
    names: &'static [&'static str],
    values: Vec<Option<Bound<'py, PyAny>>>,
    error: fn(String) -> PyErr,
}

impl<'py> Arguments<'py> {
    pub(crate) fn new(
        args: &Bound<'py, PyTuple>,
        kwargs: Option<&Bound<'py, PyDict>>,
        names: &'static [&'static str],
        positional: usize,
        error: fn(String) -> PyErr,
    ) -> PyResult<Self> {
        if args.len() > positional {
            return Err(error(format!(
                "expected at most {positional} positional arguments"
            )));
        }
        let mut values = vec![None; names.len()];
        for (index, value) in args.iter().enumerate() {
            values[index] = Some(value);
        }
        if let Some(kwargs) = kwargs {
            for (key, value) in kwargs.iter() {
                let name = key
                    .extract::<String>()
                    .map_err(|_| error("keyword names must be strings".into()))?;
                let index = names
                    .iter()
                    .position(|item| *item == name)
                    .ok_or_else(|| error(format!("unexpected argument '{name}'")))?;
                if values[index].is_some() {
                    return Err(error(format!("multiple values for '{name}'")));
                }
                values[index] = Some(value);
            }
        }
        Ok(Self {
            names,
            values,
            error,
        })
    }

    pub(crate) fn value(&self, name: &str) -> Option<&Bound<'py, PyAny>> {
        self.values[self
            .names
            .iter()
            .position(|item| *item == name)
            .expect("known argument")]
        .as_ref()
    }

    pub(crate) fn required<T>(&self, name: &str) -> PyResult<T>
    where
        T: for<'a> FromPyObject<'a, 'py>,
    {
        let value = self
            .value(name)
            .ok_or_else(|| (self.error)(format!("missing required argument '{name}'")))?;
        // Do not include arbitrary object representations or credential values.
        value.extract::<T>().map_err(|error| {
            let error: PyErr = error.into();
            if !error.is_instance_of::<pyo3::exceptions::PyException>(value.py())
                || error.is_instance_of::<pyo3::exceptions::PyMemoryError>(value.py())
            {
                // Interrupts, process exit and allocation failures are not bad arguments.
                error
            } else {
                (self.error)(format!("invalid type or value for '{name}'"))
            }
        })
    }

    pub(crate) fn optional<T>(&self, name: &str) -> PyResult<Option<T>>
    where
        T: for<'a> FromPyObject<'a, 'py>,
    {
        match self.value(name) {
            None => Ok(None),
            Some(value) if value.is_none() => Ok(None),
            Some(_) => self.required(name).map(Some),
        }
    }

    pub(crate) fn default<T>(&self, name: &str, default: T) -> PyResult<T>
    where
        T: for<'a> FromPyObject<'a, 'py>,
    {
        if self.value(name).is_none() {
            Ok(default)
        } else {
            self.required(name)
        }
    }
}
