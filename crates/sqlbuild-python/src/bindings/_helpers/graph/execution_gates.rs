//! Ordering edges attached audits and model hooks add, and the keys hooks and SQL refs name.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::records::construct::{enum_from_value, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::RecordId;
/// The model config keys whose entries are hooks, in lifecycle order.
const MODEL_HOOK_KEYS: [&str; 2] = ["pre_hooks", "post_hooks"];

fn keyword_call<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        keywords.set_item(name, value)?;
    }
    callable.call((), Some(&keywords))
}

/// The Python hooks attached to one model, in lifecycle order without repeats.
#[pyfunction]
#[pyo3(signature = (*, model))]
pub(crate) fn model_python_hook_names<'py>(
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

/// The compiled graph key a typed model, source or seed reference names.
#[pyfunction]
pub(crate) fn sql_ref_key<'py>(
    py: Python<'py>,
    reference: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let resource_type: Bound<'py, PyAny> = enum_from_value(
        py,
        RecordId::CompiledResourceType,
        native_field(reference, "kind")?.getattr("value")?,
    )?;
    keyword_call(
        &record_class(py, RecordId::CompiledObjectKey)?,
        &[
            ("resource_type", resource_type),
            ("name", native_field(reference, "name")?),
        ],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(model_python_hook_names, module)?)?;
    module.add_function(wrap_pyfunction!(sql_ref_key, module)?)?;
    Ok(())
}
