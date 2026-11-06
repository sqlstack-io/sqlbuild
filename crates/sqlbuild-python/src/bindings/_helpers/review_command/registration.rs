//! Register the review schema name parser the janitor uses.

use pyo3::prelude::{Bound, PyAny, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::review_command::main::parse_review_schema_name::parse_review_schema_name as parse_name;

use crate::bindings::_helpers::janitor::janitor_keys::utc_datetime;

/// The aware UTC creation time a strict review schema name encodes, or `None`.
#[pyfunction]
#[pyo3(signature = (name, /))]
pub(crate) fn parse_review_schema_name<'py>(
    py: Python<'py>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    parse_name(name)
        .map(|timestamp| utc_datetime(py, timestamp))
        .transpose()
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse_review_schema_name, module)?)
}
