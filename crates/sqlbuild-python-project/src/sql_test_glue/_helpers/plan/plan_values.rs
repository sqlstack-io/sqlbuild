//! Python value boundaries shared by SQL-test plan assembly.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyString;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) fn string<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

pub(crate) fn model_payload(test: &Bound<'_, PyAny>) -> PyResult<bool> {
    native_field(test, "payload")?.is_instance(&record_class(
        test.py(),
        RecordId::CompiledModelSqlTestPayload,
    )?)
}

pub(crate) fn tuple<'py>(value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    imported(value.py(), "builtins", "tuple")?.call1((value,))
}
