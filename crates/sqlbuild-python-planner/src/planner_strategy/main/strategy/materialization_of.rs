//! The `MaterializationType` of a model's `materialized` config: table when unset, custom when unknown.

use crate::planner_strategy::_helpers::strategy;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The `MaterializationType` of a model's `materialized` config: table when unset, custom when unknown.
#[inline]
pub fn materialization_of<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    strategy::materialization_of(model)
}
