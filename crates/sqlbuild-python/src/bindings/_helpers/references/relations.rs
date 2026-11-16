//! Relation names in SQL and the project relations they match, for hard-coded name checks.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::text::main::python_casefold::python_casefold;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::analysis::relation_names::{
    RelationRow, extract_sql_relation_names,
};
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

const RUNTIME_RELATION_PREFIX: &str = "SELECT 1 FROM ";

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

fn relation_name<'py>(
    py: Python<'py>,
    (name, schema, database): RelationRow,
) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("name", name)?;
    keywords.set_item("schema", schema)?;
    keywords.set_item("database", database)?;
    host_entry(py, ExecutorHost::RelationName)?.call((), Some(&keywords))
}

/// Relations named in `sql` and temporary tables it creates, or None if unparseable.
#[pyfunction]
#[pyo3(signature = (*, sql, dialect))]
fn extract_relation_names<'py>(
    py: Python<'py>,
    sql: &str,
    dialect: Option<&str>,
) -> PyResult<Option<Bound<'py, PyTuple>>> {
    let Some((relations, temporary)) = extract_sql_relation_names(py, sql, dialect)? else {
        return Ok(None);
    };
    let names: Vec<Bound<'py, PyAny>> = relations
        .into_iter()
        .map(|row| relation_name(py, row))
        .collect::<PyResult<_>>()?;
    Ok(Some(PyTuple::new(
        py,
        [
            PyTuple::new(py, names)?.into_any(),
            PyFrozenSet::new(py, temporary)?.into_any(),
        ],
    )?))
}

/// Whether two optional qualifier parts can name the same relation.
fn parts_compatible(left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => python_casefold(left) == python_casefold(right),
        _ => true,
    }
}

/// The project relation `relation` names, resolving missing qualifiers from the defaults.
#[pyfunction]
#[pyo3(signature = (*, index, relation, default_database = None, default_schema = None))]
pub(crate) fn match_project_relation<'py>(
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

/// Project relations from adapter-rendered runtime relation names, deduplicated in order.
#[pyfunction]
#[pyo3(signature = (*, relations, dialect))]
fn runtime_project_relations<'py>(
    py: Python<'py>,
    relations: &Bound<'py, PyAny>,
    dialect: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let relation_type: Bound<'py, PyAny> = host_entry(py, ExecutorHost::ProjectRelation)?;
    let resolved: Bound<'py, PyDict> = PyDict::new(py);
    for item in mapping_items(relations)?.try_iter()? {
        let (reference, qualified): (Bound<'py, PyAny>, String) = item?.extract()?;
        let sql: String = format!("{RUNTIME_RELATION_PREFIX}{qualified}");
        let Some((mut names, _)) = extract_sql_relation_names(py, &sql, dialect)? else {
            continue;
        };
        if names.len() != 1 {
            continue;
        }
        let Some(row) = names.pop() else {
            continue;
        };
        let relation: Bound<'py, PyAny> = keyword_call(
            &relation_type,
            &[("ref", reference), ("relation", relation_name(py, row)?)],
        )?;
        resolved.set_item(relation, py.None())?;
    }
    keyword_call(
        &host_entry(py, ExecutorHost::ProjectRelationIndex)?,
        &[("relations", PyTuple::new(py, resolved.keys())?.into_any())],
    )
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(extract_relation_names, module)?)?;
    module.add_function(wrap_pyfunction!(match_project_relation, module)?)?;
    module.add_function(wrap_pyfunction!(runtime_project_relations, module)?)?;
    Ok(())
}
