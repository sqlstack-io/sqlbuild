//! Small Python value helpers and callback relays the build command shares.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{
    PyBool, PyDict, PyDictMethods, PyFloat, PyFrozenSet, PyInt, PyString, PyTuple, PyTupleMethods,
};
use pyo3::{IntoPyObject, pyclass, pymethods};

use crate::boundary::main::imported::imported;

const DATETIME_MODULE: &str = "datetime";

pub(crate) fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

pub(crate) fn boolean(py: Python<'_>, value: bool) -> Bound<'_, PyAny> {
    PyBool::new(py, value).to_owned().into_any()
}

pub(crate) fn none(py: Python<'_>) -> Bound<'_, PyAny> {
    py.None().into_bound(py)
}

pub(crate) fn tuple<'py>(
    py: Python<'py>,
    values: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, values)?.into_any())
}

pub(crate) fn frozenset<'py>(
    py: Python<'py>,
    values: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyFrozenSet::new(py, values)?.into_any())
}

/// The value, or `default` when it is `None`.
pub(crate) fn or_none<'py>(
    value: Bound<'py, PyAny>,
    default: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    if value.is_none() {
        return default();
    }
    Ok(value)
}

/// `base / part`.
pub(crate) fn joined<'py>(base: &Bound<'py, PyAny>, parts: &[&str]) -> PyResult<Bound<'py, PyAny>> {
    let mut path: Bound<'py, PyAny> = base.clone();
    for part in parts {
        path = path.call_method1("__truediv__", (*part,))?;
    }
    Ok(path)
}

/// Write `value` to the stream and flush it.
pub(crate) fn emit(stream: &Bound<'_, PyAny>, value: &str) -> PyResult<()> {
    stream.call_method1("write", (value,))?;
    stream.call_method0("flush")?;
    Ok(())
}

/// `datetime.now(UTC)`.
pub(crate) fn utc_now(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    let utc: Bound<'_, PyAny> = imported(py, DATETIME_MODULE, "UTC")?;
    imported(py, DATETIME_MODULE, "datetime")?.call_method1("now", (utc,))
}

/// A string, integer or float shown as text; anything else is unset.
pub(crate) fn display_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_instance_of::<PyBool>()
        || !(value.is_instance_of::<PyString>()
            || value.is_instance_of::<PyInt>()
            || value.is_instance_of::<PyFloat>())
    {
        return Ok(None);
    }
    Ok(Some(value.str()?.extract()?))
}

/// Forward positional callback arguments to a callable that takes them by keyword.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct KeywordRelay {
    target: Py<PyAny>,
    names: Vec<&'static str>,
}

#[pymethods]
impl KeywordRelay {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        py: Python<'_>,
        args: Bound<'_, PyTuple>,
        kwargs: Option<Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        for (name, value) in self.names.iter().zip(args.iter()) {
            options.set_item(*name, value)?;
        }
        if let Some(kwargs) = kwargs {
            options.update(kwargs.as_mapping())?;
        }
        self.target.bind(py).call((), Some(&options))?;
        Ok(())
    }
}

/// A relay calling `target` with `names` as keywords.
pub(crate) fn keyword_relay<'py>(
    target: Bound<'py, PyAny>,
    names: &[&'static str],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target.py();
    Ok(KeywordRelay {
        target: target.unbind(),
        names: names.to_vec(),
    }
    .into_pyobject(py)?
    .into_any())
}

/// Write each message as one flushed line.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct ProgressLineWriter {
    stream: Py<PyAny>,
}

#[pymethods]
impl ProgressLineWriter {
    fn __call__(&self, py: Python<'_>, message: &str) -> PyResult<()> {
        emit(self.stream.bind(py), &format!("{message}\n"))
    }
}

/// A callable writing each message as a line on `stream`.
pub(crate) fn line_writer<'py>(stream: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    Ok(ProgressLineWriter {
        stream: stream.clone().unbind(),
    }
    .into_pyobject(stream.py())?
    .into_any())
}
