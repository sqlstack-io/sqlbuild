//! The Python hooks attached to one model, in lifecycle order without repeats.

use crate::graph::_helpers::nodes::execution_gates::MODEL_HOOK_KEYS;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyList, PyTuple};
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;

/// The Python hooks attached to one model, in lifecycle order without repeats.
#[pyfunction]
#[pyo3(signature = (*, model))]
pub fn model_python_hook_names<'py>(
    py: Python<'py>,
    model: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let entry_type: Bound<'_, PyAny> = record_class(py, RecordId::PythonHookEntry)?;
    let values: Bound<'_, PyAny> = native_field(model, "config")?.getattr("values")?;
    let mut names: Vec<String> = Vec::new();
    for hook_key in MODEL_HOOK_KEYS {
        let entries: Bound<'_, PyAny> = mapping_get(&values, hook_key)?;
        if entries.is_instance(&entry_type)? {
            names.push(native_field(&entries, "name")?.extract()?);
        } else if entries.is_instance_of::<PyList>() || entries.is_instance_of::<PyTuple>() {
            for entry in entries.try_iter()? {
                let entry: Bound<'_, PyAny> = entry?;
                if entry.is_instance(&entry_type)? {
                    names.push(native_field(&entry, "name")?.extract()?);
                }
            }
        }
    }
    let mut unique: Vec<String> = Vec::with_capacity(names.len());
    for name in names {
        if !unique.contains(&name) {
            unique.push(name);
        }
    }
    PyTuple::new(py, unique)
}
