//! The process environment as compile text reads it, rejecting values that are not UTF-8.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBytes, PyBytesMethods, PyDict, PyDictMethods, PyString};
use pyo3::{pyclass, pymethods};

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::types::ErrorId;

/// `os.environ`, raising a clear error for a value holding undecodable bytes.
#[pyclass(module = "sqlbuild._native", frozen, mapping)]
pub(crate) struct UnicodeEnvironment;

fn environ(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, "os", "environ")
}

#[pymethods]
impl UnicodeEnvironment {
    #[new]
    fn new() -> Self {
        Self
    }

    fn __getitem__<'py>(&self, name: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
        let py: Python<'py> = name.py();
        let value: Bound<'py, PyAny> = environ(py)?.get_item(name)?;
        let encoded: Bound<'py, PyBytes> = value
            .cast::<PyString>()?
            .call_method1("encode", ("utf-8", "surrogatepass"))?
            .cast_into::<PyBytes>()?;
        let byte_offset: usize = match std::str::from_utf8(encoded.as_bytes()) {
            Ok(_) => return Ok(value),
            Err(error) => error.valid_up_to(),
        };
        let name: String = name.str()?.to_string();
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item(
            "help",
            format!(
                "Set '{name}' to UTF-8 text, since SQLBuild cannot send undecodable bytes to a \
                 warehouse; the value is not shown because it may be a secret"
            ),
        )?;
        Err(PyErr::from_value(
            error_class(py, ErrorId::CompileInput)?.call(
                (format!(
                    "Environment variable '{name}' is not valid UTF-8 text: byte {byte_offset} \
                     starts an invalid UTF-8 sequence"
                ),),
                Some(&keywords),
            )?,
        ))
    }

    fn __contains__(&self, name: &Bound<'_, PyAny>) -> PyResult<bool> {
        environ(name.py())?.contains(name)
    }

    fn __iter__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        environ(py)?.try_iter().map(Bound::into_any)
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        environ(py)?.len()
    }
}
