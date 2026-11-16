//! Python boundary values shared by plan entry and plan output assembly.

use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::PyString;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

pub(super) const MICROBATCH_CONCURRENCY_MESSAGE: &str =
    "batch_concurrency > 1 requires concurrent microbatches; selected model(s): ";

/// The native record `name(**pairs)`.
pub(super) fn native<'py>(
    py: Python<'py>,
    name: RecordId,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, name, pairs)
}

/// The planner record `name(**pairs)`.
pub(super) fn planner_model<'py>(
    py: Python<'py>,
    name: RecordId,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, name, pairs)
}

/// One member of a planner enumeration.
pub(super) fn planner_type<'py>(
    py: Python<'py>,
    class: RecordId,
    member: &str,
) -> PyResult<Bound<'py, PyAny>> {
    enum_member(py, class, member)
}

/// A Python string.
pub(super) fn string<'py>(py: Python<'py>, text: &str) -> Bound<'py, PyAny> {
    PyString::new(py, text).into_any()
}

/// A Python bool.
pub(super) fn boolean(py: Python<'_>, value: bool) -> PyResult<Bound<'_, PyAny>> {
    Ok(value.into_pyobject(py)?.to_owned().into_any())
}

/// `None`.
pub(super) fn none(py: Python<'_>) -> Bound<'_, PyAny> {
    py.None().into_bound(py)
}

/// `dataclasses.replace(object, **fields)`.
pub(super) fn replaced<'py>(
    object: &Bound<'py, PyAny>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    replaced_keywords(object, &keywords(object.py(), fields)?)
}

/// `class(message, **fields)` from `module`, as a raised error.
pub(super) fn error_of(
    py: Python<'_>,
    class: ErrorId,
    message: &str,
    fields: &[(&str, Bound<'_, PyAny>)],
) -> PyErr {
    match error_class(py, class)
        .and_then(|class| class.call((message,), Some(&keywords(py, fields)?)))
    {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}
