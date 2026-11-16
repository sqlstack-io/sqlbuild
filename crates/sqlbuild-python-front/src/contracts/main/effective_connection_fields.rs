//! Catalog selection uses the same named-layer resolver, with no writer layers.

use crate::contracts::_helpers::effective_config;
use crate::contracts::_helpers::effective_config::Outcome;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyDict;

/// Catalog selection uses the same named-layer resolver, with no writer layers.
#[inline]
pub fn effective_connection_fields<'py>(
    py: Python<'py>,
    (project_config, local_config): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    target_config: &Bound<'py, PyAny>,
    catalog: bool,
) -> PyResult<Outcome<'py, Bound<'py, PyDict>>> {
    effective_config::effective_connection_fields(
        py,
        (project_config, local_config),
        target_config,
        catalog,
    )
}
