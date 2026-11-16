//! Model lifecycle hooks as the plain dictionaries manifests and identities record.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyList, PyListMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const RELATIVE_PATH_FIELD: &str = "relative_path";
const SQL_HOOK_FIELDS: [&str; 6] = [
    "statement",
    "name",
    "relative_path",
    "definition_sql",
    "kwargs",
    "description",
];

fn sql_hook<'py>(entry: &Bound<'py, PyAny>, sql_fields: &[String]) -> PyResult<Bound<'py, PyDict>> {
    let hook: Bound<'py, PyDict> = PyDict::new(entry.py());
    hook.set_item("type", "sql")?;
    for field in sql_fields {
        let Some(name) = SQL_HOOK_FIELDS.iter().find(|name| **name == field) else {
            return Err(pyo3::exceptions::PyKeyError::new_err(field.clone()));
        };
        let mut value: Bound<'py, PyAny> = native_field(entry, name)?;
        if value.is_none() {
            continue;
        }
        if *name == RELATIVE_PATH_FIELD {
            value = value.call_method0("as_posix")?;
        }
        hook.set_item(*name, value)?;
    }
    Ok(hook)
}

/// Serialize SQL and Python hook entries; other values serialize to no hooks.
pub(crate) fn hook_entries<'py>(
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

#[pyfunction]
#[pyo3(signature = (*, value, sql_fields, python_hook_fields = None))]
fn serialize_hook_entries<'py>(
    value: &Bound<'py, PyAny>,
    sql_fields: Vec<String>,
    python_hook_fields: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    hook_entries(value, &sql_fields, python_hook_fields.as_ref())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(serialize_hook_entries, module)?)?;
    Ok(())
}
