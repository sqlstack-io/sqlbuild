//! Model lifecycle hooks as the plain dictionaries manifests and identities record.

use crate::discovery::main::headers::hook_entries::hook_entries;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyList};

use sqlbuild_python_base::records::main::fields::field::field as native_field;

const RELATIVE_PATH_FIELD: &str = "relative_path";
const SQL_HOOK_FIELDS: [&str; 6] = [
    "statement",
    "name",
    "relative_path",
    "definition_sql",
    "kwargs",
    "description",
];

pub(crate) fn sql_hook<'py>(
    entry: &Bound<'py, PyAny>,
    sql_fields: &[String],
) -> PyResult<Bound<'py, PyDict>> {
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

#[pyfunction]
#[pyo3(signature = (*, value, sql_fields, python_hook_fields = None))]
pub(crate) fn serialize_hook_entries<'py>(
    value: &Bound<'py, PyAny>,
    sql_fields: Vec<String>,
    python_hook_fields: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyList>> {
    hook_entries(value, &sql_fields, python_hook_fields.as_ref())
}
