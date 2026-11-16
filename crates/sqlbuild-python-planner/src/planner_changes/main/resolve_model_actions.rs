//! Each selected model's change with the backfill its own `replay_on_change` decided.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::planner_scope::constants::MODEL;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;

/// Each selected model's change with the backfill its own `replay_on_change` decided.
pub fn resolve_model_actions<'py>(
    scope: &Bound<'py, PyAny>,
    changes: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = scope.py();
    let scope = scope_data(scope)?;
    let model_changes: Bound<'py, PyAny> = native_field(changes, "models")?;
    let resolved: Bound<'py, PyDict> = PyDict::new(py);
    for key in &scope.order {
        if !scope.selected.contains(key) || key.0 != MODEL {
            continue;
        }
        let name = &key.1;
        let change: Bound<'py, PyAny> = mapping_get(&model_changes, name)?;
        if !scope.models.contains_key(name) || change.is_none() {
            continue;
        }
        let backfill: Bound<'py, PyAny> = native_field(&change, "backfill")?;
        resolved.set_item(
            name,
            new_record_with(
                py,
                RecordId::ResolvedModelAction,
                &[("change", change), ("backfill", backfill)],
            )?,
        )?;
    }
    new_record_with(
        py,
        RecordId::PlannerResolvedActions,
        &[("models", resolved.into_any())],
    )
}
