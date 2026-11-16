//! The hash of a model's own declared columns, column families and contract setting.

use crate::planner_identity::_helpers::metadata::column_signature;
use crate::planner_identity::_helpers::metadata::family_signatures;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods};
use sqlbuild_analysis::planner_identity::constants::CONTRACT_CONFIG_KEY;
use sqlbuild_core::fingerprint_identity::main::stable_identity_hash::stable_identity_hash;
use sqlbuild_python_base::planner_identity::constants::COMPACT;
use sqlbuild_python_base::planner_identity::main::sorted_dumps::sorted_dumps;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::model_config::main::config::config_value::config_value;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

/// The hash of a model's own declared columns, column families and contract setting.
pub fn model_declared_columns_hash(model: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = model.py();
    let schema_entry: Bound<'_, PyAny> =
        sqlbuild_python_project::compiled_views::main::compiled_models::schema_entry::schema_entry(
            model,
        )?;
    let payload: Bound<'_, PyDict> = PyDict::new(py);
    payload.set_item(
        "contract",
        config_value(py, &model_values(model)?, CONTRACT_CONFIG_KEY)?,
    )?;
    let columns: Bound<'_, PyList> = PyList::empty(py);
    let mut families: Bound<'_, PyList> = PyList::empty(py);
    let mut type_enforcement: Bound<'_, PyAny> = py.None().into_bound(py);
    if !schema_entry.is_none() {
        for column in native_field(&schema_entry, "columns")?.try_iter()? {
            columns.append(column_signature(&column?)?)?;
        }
        families = family_signatures(&native_field(&schema_entry, "dynamic_columns")?)?;
        type_enforcement = native_field(&schema_entry, "type_enforcement")?;
    }
    payload.set_item("columns", columns)?;
    payload.set_item("dynamic_columns", families)?;
    payload.set_item("type_enforcement", type_enforcement)?;
    Ok(stable_identity_hash(&sorted_dumps(
        payload.as_any(),
        COMPACT,
        true,
    )?))
}
