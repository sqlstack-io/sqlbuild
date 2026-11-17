//! The `MaterializationType` of a model's `materialized` config: table when unset, custom when unknown.

use crate::planner_changes::main::planner_enum::planner_enum;
use crate::planner_strategy::_helpers::values::{materialization_value, model_materialization};
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;
use sqlbuild_python_base::records::types::RecordId;

/// The `MaterializationType` of a model's `materialized` config: table when unset, custom when unknown.
#[pyfunction(name = "get_materialization_type")]
#[pyo3(signature = (model))]
pub fn materialization_of<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    planner_enum(
        model.py(),
        RecordId::MaterializationType,
        materialization_value(model_materialization(model)?),
    )
}
