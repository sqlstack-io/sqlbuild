//! Serialize SQL and Python hook entries; other values serialize to no hooks.

use crate::discovery::_helpers::headers::hook_entries::sql_hook;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

/// Serialize SQL and Python hook entries; other values serialize to no hooks.
pub fn hook_entries<'py>(
    value: &Bound<'py, PyAny>,
    sql_fields: &[String],
    python_hook_fields: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    let py = value.py();
    let hooks: Bound<'py, PyList> = PyList::empty(py);
    if !value.is_instance_of::<PyList>() && !value.is_instance_of::<PyTuple>() {
        return Ok(hooks);
    }
    let sql_class: Bound<'py, PyAny> = record_class(py, RecordId::SqlHookEntry)?;
    let python_class: Bound<'py, PyAny> = record_class(py, RecordId::PythonHookEntry)?;
    for entry in value.try_iter()? {
        let entry: Bound<'py, PyAny> = entry?;
        if entry.is_instance(&sql_class)? {
            hooks.append(sql_hook(&entry, sql_fields)?)?;
        } else if entry.is_instance(&python_class)? {
            let hook: Bound<'py, PyDict> = PyDict::new(py);
            let name: Bound<'py, PyAny> = native_field(&entry, "name")?;
            hook.set_item("type", "python")?;
            hook.set_item("name", &name)?;
            hook.set_item("kwargs", native_field(&entry, "kwargs")?)?;
            if let Some(fields) = python_hook_fields
                && let Some(extra) = fields.get_item(&name)?
            {
                hook.call_method1("update", (extra,))?;
            }
            hooks.append(hook)?;
        }
    }
    Ok(hooks)
}
