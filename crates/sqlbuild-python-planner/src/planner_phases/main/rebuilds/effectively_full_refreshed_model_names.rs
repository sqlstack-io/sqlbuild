//! Names of models effectively full refreshed for this invocation.

use crate::planner_phases::main::rebuilds::resolve_model_full_refresh::resolve_model_full_refresh;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_project::compiled_views::main::compiled_models::name_object::name_object;
use sqlbuild_python_project::compiled_views::main::project::compiled_models::compiled_models;
use sqlbuild_python_project::planner_phases::main::frozenset::frozenset;

/// Names of models effectively full refreshed for this invocation.
pub fn effectively_full_refreshed_model_names<'py>(
    project: &Bound<'py, PyAny>,
    cli_full_refresh: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let mut names: Vec<_> = Vec::new();
    for model in compiled_models(project)?.try_iter()? {
        let model = model?;
        if resolve_model_full_refresh(&model, cli_full_refresh)? {
            names.push(name_object(&model)?);
        }
    }
    frozenset(project.py(), &names)
}
