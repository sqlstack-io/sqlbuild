//! The unselected models the selected scope reads, sorted.

use crate::planner_phases::_helpers::inputs::contexts::class_field;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_analysis::planner_scope::constants::MODEL;
use sqlbuild_python_project::planner_scope::models::PlannerScopeResolution;

/// The unselected models the selected scope reads, sorted.
pub fn unselected_upstream_models(scopes: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let selected = class_field(scopes, |s: &PlannerScopeResolution| &s.selected_scope)?;
    let selected = sqlbuild_python_project::planner_scope::main::scope_data::scope_data(&selected)?;
    let mut names: Vec<String> = Vec::new();
    for (key, deps) in &selected.upstream {
        if !selected.selected.contains(key) {
            continue;
        }
        for dep in deps {
            if dep.0 == MODEL && !selected.selected.contains(dep) {
                names.push(dep.1.clone());
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}
