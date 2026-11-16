//! Native record classes and string enums held natively, so records are built without a module lookup.

use pyo3::IntoPyObject;
use pyo3::call::PyCallArgs;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyResult, Python};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods};

use crate::boundary::main::keywords::keywords;
use crate::records::types::{ErrorId, RecordId};

/// Each `RecordId`'s class, captured when the extension module is initialised.
static CLASSES: PyOnceLock<Vec<Py<PyAny>>> = PyOnceLock::new();

/// Each `ErrorId`'s exception class, captured when the extension module is initialised.
static ERROR_CLASSES: PyOnceLock<Vec<Py<PyAny>>> = PyOnceLock::new();

/// Hold every registered record class, string enum and error class, read once while the module initialises.
pub(crate) fn capture(module: &Bound<'_, PyModule>) -> PyResult<()> {
    CLASSES.get_or_try_init(module.py(), || {
        RecordId::ALL
            .iter()
            .map(|id| module.getattr(id.name()).map(Bound::unbind))
            .collect::<PyResult<Vec<Py<PyAny>>>>()
    })?;
    ERROR_CLASSES.get_or_try_init(module.py(), || {
        ErrorId::ALL
            .iter()
            .map(|id| module.getattr(id.name()).map(Bound::unbind))
            .collect::<PyResult<Vec<Py<PyAny>>>>()
    })?;
    Ok(())
}

/// The native error class `id` as registered, ignoring later attribute replacement.
pub(crate) fn error_class(py: Python<'_>, id: ErrorId) -> PyResult<Bound<'_, PyAny>> {
    let classes: &Vec<Py<PyAny>> = ERROR_CLASSES
        .get(py)
        .ok_or_else(|| PyRuntimeError::new_err("native error classes are not registered"))?;
    Ok(classes[id as usize].bind(py).clone())
}

/// The native `id` exception raised with `message`, or the error looking it up or raising it.
pub(crate) fn new_plain_error(py: Python<'_>, id: ErrorId, message: &str) -> PyErr {
    new_error(py, id, message, (None, None))
}

/// The native `id` exception with `message` and the given `code` and `help`, or the error raising it.
pub(crate) fn new_error(
    py: Python<'_>,
    id: ErrorId,
    message: &str,
    keywords: (Option<&str>, Option<&str>),
) -> PyErr {
    exception_from(error_class(py, id), message, keywords)
}

/// The exception `class` with `message` and the given `code` and `help`, or the error raising it.
pub(crate) fn exception_from(
    class: PyResult<Bound<'_, PyAny>>,
    message: &str,
    (code, help): (Option<&str>, Option<&str>),
) -> PyErr {
    class
        .and_then(|class| {
            let keywords: Bound<'_, PyDict> = PyDict::new(class.py());
            if let Some(code) = code {
                keywords.set_item("code", code)?;
            }
            if let Some(help) = help {
                keywords.set_item("help", help)?;
            }
            class.call((message,), Some(&keywords))
        })
        .map_or_else(|error| error, PyErr::from_value)
}

/// A raisable native `id` error carrying `message`, with each `(name, value)` attribute set on it.
pub(crate) fn new_annotated_error(
    py: Python<'_>,
    id: ErrorId,
    message: String,
    attributes: &[(&str, String)],
) -> PyErr {
    let raised = (|| {
        let error: Bound<'_, PyAny> = error_class(py, id)?.call1((message,))?;
        for (name, value) in attributes {
            error.setattr(*name, value)?;
        }
        Ok::<_, PyErr>(error)
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// The native record class or string enum `id` as registered, ignoring later attribute replacement.
pub(crate) fn record_class(py: Python<'_>, id: RecordId) -> PyResult<Bound<'_, PyAny>> {
    let classes: &Vec<Py<PyAny>> = CLASSES
        .get(py)
        .ok_or_else(|| PyRuntimeError::new_err("native record classes are not registered"))?;
    Ok(classes[id as usize].bind(py).clone())
}

/// A new `id` record built exactly as calling its class builds it, finishers included.
pub(crate) fn new_record<'py, A: PyCallArgs<'py>>(
    py: Python<'py>,
    id: RecordId,
    args: A,
    kwargs: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    record_class(py, id)?.call(args, kwargs)
}

/// A new `id` record built from `(field, value)` keyword pairs.
pub(crate) fn new_record_with<'py>(
    py: Python<'py>,
    id: RecordId,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record(py, id, (), Some(&keywords(py, pairs)?))
}

/// The member `name` of the native string enum `id`, looked up rather than constructed.
pub(crate) fn enum_member<'py>(
    py: Python<'py>,
    id: RecordId,
    name: &str,
) -> PyResult<Bound<'py, PyAny>> {
    record_class(py, id)?.getattr(name)
}

/// The member of the native string enum `id` whose value is `value`, as `Enum(value)` finds it.
pub(crate) fn enum_from_value<'py, V: IntoPyObject<'py>>(
    py: Python<'py>,
    id: RecordId,
    value: V,
) -> PyResult<Bound<'py, PyAny>> {
    record_class(py, id)?.call1((value,))
}
