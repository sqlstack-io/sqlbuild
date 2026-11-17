//! One model's pre and post hooks with their SQL resolved.

use crate::planner_resolution::models::Resolution;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;
use sqlbuild_python_project::compiled_views::main::compiled_models::model_name::model_name;

/// One model's pre and post hooks with their SQL resolved.
pub fn model_hook_entries<'py>(
    model: &Bound<'py, PyAny>,
    resolution: &Resolution<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = model.py();
    let values = model_values(model)?;
    let name: String = model_name(model)?;
    let mut hooks: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in ["pre_hooks", "post_hooks"] {
        hooks.push(resolution.hook(
            config_value(py, &values, key)?,
            &format!("Model '{name}' {key}"),
        )?);
    }
    PyTuple::new(model.py(), hooks)
}
