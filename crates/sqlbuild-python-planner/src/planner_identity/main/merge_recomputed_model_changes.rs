//! Resolved actions with recomputed model changes patched in where available.

use crate::planner_identity::_helpers::identities::replace_with;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;

/// Resolved actions with recomputed model changes patched in where available.
#[pyfunction]
#[pyo3(signature = (*, resolved_actions, changes))]
pub fn merge_recomputed_model_changes<'py>(
    resolved_actions: &Bound<'py, PyAny>,
    changes: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = resolved_actions.py();
    let recomputed: Bound<'py, PyAny> = native_field(changes, "models")?;
    let merged: Bound<'py, PyDict> = PyDict::new(py);
    for item in native_field(resolved_actions, "models")?
        .call_method0("items")?
        .try_iter()?
    {
        let (name, resolved): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        let mut change: Bound<'py, PyAny> = mapping_get(&recomputed, &name)?;
        if change.is_none() {
            change = native_field(&resolved, "change")?;
        }
        merged.set_item(name, replace_with(&resolved, ("change", change))?)?;
    }
    replace_with(resolved_actions, ("models", merged.into_any()))
}
