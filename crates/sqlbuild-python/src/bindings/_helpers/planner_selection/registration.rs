//! Register planner selection bindings.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_project::planner_selection::main::register_model_selectors::register_model_selectors;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_model_selectors(module)?;
    Ok(())
}
