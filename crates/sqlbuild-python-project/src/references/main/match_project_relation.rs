//! The project relation `relation` names, resolving missing qualifiers from the defaults.

use crate::references::_helpers::relations::parts_compatible;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use sqlbuild_core::text::main::python_casefold::python_casefold;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The project relation `relation` names, resolving missing qualifiers from the defaults.
#[pyfunction]
#[pyo3(signature = (*, index, relation, default_database = None, default_schema = None))]
pub fn match_project_relation<'py>(
    index: &Bound<'py, PyAny>,
    relation: &Bound<'py, PyAny>,
    default_database: Option<String>,
    default_schema: Option<String>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let name: String = python_casefold(&native_field(relation, "name")?.extract::<String>()?);
    let schema: Option<String> = native_field(relation, "schema")?
        .extract::<Option<String>>()?
        .or(default_schema);
    let database: Option<String> = native_field(relation, "database")?
        .extract::<Option<String>>()?
        .or(default_database);
    for candidate in native_field(index, "relations")?.try_iter()? {
        let candidate: Bound<'py, PyAny> = candidate?;
        let target: Bound<'py, PyAny> = native_field(&candidate, "relation")?;
        if python_casefold(&native_field(&target, "name")?.extract::<String>()?) != name {
            continue;
        }
        let target_schema: Option<String> = native_field(&target, "schema")?.extract()?;
        let target_database: Option<String> = native_field(&target, "database")?.extract()?;
        if parts_compatible(schema.as_deref(), target_schema.as_deref())
            && parts_compatible(database.as_deref(), target_database.as_deref())
        {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}
