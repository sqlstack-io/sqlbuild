//! Keyword arguments of native entry points and the transient status line they report to.

use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString};

use crate::boundary::_helpers::python_calls::call_with;

/// Keyword arguments of one call, each required.
#[derive(Debug)]
pub struct Request<'a, 'py> {
    function: &'static str,
    arguments: &'a Bound<'py, PyDict>,
}

impl<'a, 'py> Request<'a, 'py> {
    pub fn new(
        function: &'static str,
        arguments: Option<&'a Bound<'py, PyDict>>,
    ) -> PyResult<Self> {
        match arguments {
            Some(arguments) => Ok(Self {
                function,
                arguments,
            }),
            None => Err(PyTypeError::new_err(format!(
                "{function}() missing required keyword-only arguments"
            ))),
        }
    }

    pub fn py(&self) -> Python<'py> {
        self.arguments.py()
    }

    pub fn get(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        self.arguments.get_item(name)?.ok_or_else(|| {
            PyTypeError::new_err(format!(
                "{}() missing 1 required keyword-only argument: '{name}'",
                self.function
            ))
        })
    }

    /// The argument, or `None` when the caller left it out.
    pub fn optional(&self, name: &str) -> PyResult<Option<Bound<'py, PyAny>>> {
        self.arguments.get_item(name)
    }
}

/// The transient status line, when one is shown.
#[derive(Debug)]
pub struct Status<'py>(pub Bound<'py, PyAny>);

impl Status<'_> {
    pub fn start(&self, message: &str) -> PyResult<()> {
        if !self.0.is_none() {
            self.0.call_method1("start", (message,))?;
        }
        Ok(())
    }

    pub fn complete(&self, message: &str) -> PyResult<()> {
        if !self.0.is_none() {
            call_with(
                &self.0.getattr("complete")?,
                &[("message", PyString::new(self.0.py(), message).into_any())],
            )?;
        }
        Ok(())
    }
}

/// Keyword-only arguments of a driver entry point, checked as Python checks a signature.
#[derive(Debug)]
pub struct KeywordArguments<'py> {
    values: Bound<'py, PyDict>,
}

impl<'py> KeywordArguments<'py> {
    /// Accept `options` for `function`, rejecting missing `required` and unknown names.
    pub fn read(
        py: Python<'py>,
        function: &str,
        options: Option<Bound<'py, PyDict>>,
        (required, optional): (&[&str], &[&str]),
    ) -> PyResult<Self> {
        let values: Bound<'py, PyDict> = options.unwrap_or_else(|| PyDict::new(py));
        for name in values.keys() {
            let name: String = name.extract()?;
            if !required.contains(&name.as_str()) && !optional.contains(&name.as_str()) {
                return Err(PyTypeError::new_err(format!(
                    "{function}() got an unexpected keyword argument '{name}'"
                )));
            }
        }
        for name in required {
            if !values.contains(*name)? {
                return Err(PyTypeError::new_err(format!(
                    "{function}() missing 1 required keyword-only argument: '{name}'"
                )));
            }
        }
        Ok(Self { values })
    }

    /// Whether the argument `name` was passed.
    pub fn contains(&self, name: &str) -> PyResult<bool> {
        self.values.contains(name)
    }

    /// The argument `name`, or `None` when it was not passed.
    pub fn value(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.values.py();
        Ok(self
            .values
            .get_item(name)?
            .unwrap_or_else(|| py.None().into_bound(py)))
    }

    /// The argument `name` as a truth value, `False` when it was not passed.
    pub fn flag(&self, name: &str) -> PyResult<bool> {
        self.value(name)?.is_truthy()
    }

    /// The argument `name`, or `False` when it was not passed.
    pub fn value_or_false(&self, name: &str) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = self.values.py();
        Ok(self
            .values
            .get_item(name)?
            .unwrap_or_else(|| PyBool::new(py, false).to_owned().into_any()))
    }

    /// Whether `name` was passed.
    pub fn passed(&self, name: &str) -> PyResult<bool> {
        self.values.contains(name)
    }
}
