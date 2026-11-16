//! The model's old-name view retention, the project's when unset, or `None` when disabled.

use crate::state_rows::_helpers::migrations::migration_projections;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The model's old-name view retention, the project's when unset, or `None` when disabled.
#[inline]
pub fn old_name_retention<'py>(
    config_values: &Bound<'py, PyAny>,
    project_retention: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    migration_projections::old_name_retention(config_values, project_retention)
}
