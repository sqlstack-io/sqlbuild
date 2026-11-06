//! Per-model actions from each model's own change, and the incremental replacement predicate.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::planner_scope::constants::MODEL;

use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::RecordId;

const INCREMENTAL_MATERIALIZATION: &str = "incremental";
const CREATE_TABLE_ACTION: &str = "create_table";

/// Each selected model's change with the backfill its own `replay_on_change` decided.
pub(crate) fn resolve_model_actions<'py>(
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

/// Incremental replacements, whose bounds must ignore the destination maximum.
#[pyfunction]
#[pyo3(signature = (*, materialization_type, action))]
pub(crate) fn replaces_incremental_relation(materialization_type: &str, action: &str) -> bool {
    materialization_type == INCREMENTAL_MATERIALIZATION && action == CREATE_TABLE_ACTION
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(replaces_incremental_relation, module)?)
}
