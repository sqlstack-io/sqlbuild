//! Warehouse source relation listing and column gathering for planning.

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::native_host::synced_copy::SyncedCopyAdapter;
use crate::bindings::_helpers::observations::source_capture::capture_source_columns;
use crate::bindings::_helpers::planner_phases::objects::{frozenset, text, tuple};
use crate::bindings::_helpers::planner_warehouse::constants::{
    COLUMN_INFO, METADATA_NAME_FILTER_LIMIT, RELATION_LOOKUP,
};
use crate::bindings::_helpers::planner_warehouse::snapshot::{columns_for, list_relations};
use crate::bindings::_helpers::records::fields::field as native_field;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use std::collections::{BTreeSet, HashSet};

fn entries_of<'py>(
    project: &Bound<'py, PyAny>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    match source_entries.filter(|entries| !entries.is_none()) {
        Some(entries) => entries.try_iter()?.collect(),
        None => project_reads::sources(project)?
            .try_iter()?
            .map(|source| source_reads::source_entry(&source?))
            .collect(),
    }
}

fn table_name(entry: &Bound<'_, PyAny>) -> PyResult<String> {
    let table = native_field(entry, "table")?;
    if table.is_none() {
        native_field(entry, "name")?.extract()
    } else {
        table.extract()
    }
}

fn database_of(entry: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let database = native_field(entry, "database")?;
    if database.is_truthy()? {
        Ok(Some(database.extract()?))
    } else {
        Ok(None)
    }
}

fn name_filter(
    entries: &[Bound<'_, PyAny>],
    database: Option<&str>,
    schemas: &BTreeSet<String>,
) -> PyResult<Option<BTreeSet<String>>> {
    let mut names: BTreeSet<_> = BTreeSet::new();
    for entry in entries {
        if !native_field(entry, "expression")?.is_none() {
            continue;
        }
        let schema: Option<String> = native_field(entry, "schema")?.extract()?;
        if !schema.is_some_and(|schema| schemas.contains(&schema)) {
            continue;
        }
        if database_of(entry)?.as_deref() != database {
            continue;
        }
        names.insert(table_name(entry)?);
    }
    if names.is_empty() || names.len() > METADATA_NAME_FILTER_LIMIT {
        return Ok(None);
    }
    Ok(Some(names))
}

fn names_tuple<'py>(py: Python<'py>, names: &BTreeSet<String>) -> PyResult<Bound<'py, PyAny>> {
    tuple(py, names.iter().map(|name| text(py, name)).collect())
}

/// Source columns and the names of sources the relation listing found.
pub(crate) fn source_inspection<'py>(
    project: &Bound<'py, PyAny>,
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyAny>)> {
    let py = project.py();
    let entries = entries_of(project, source_entries)?;
    let result = PyDict::new(py);
    let mut listed: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut listed_names: HashSet<String> = HashSet::new();
    let mut groups: Vec<(String, BTreeSet<String>)> = Vec::new();
    let column_info = host::entry(py, COLUMN_INFO)?;
    for entry in &entries {
        let expression = native_field(entry, "expression")?;
        if !expression.is_none() {
            if native_field(entry, "type_enforcement")?.is_truthy()? {
                let sql: String = expression.extract()?;
                let source: Bound<'py, PyAny> = native_field(entry, "name")?;
                let source_name: String = source.extract()?;
                let described: Vec<String> = match adapter.downcast::<SyncedCopyAdapter>() {
                    Ok(facade) => match facade.get().described_columns(&source_name, &sql)? {
                        Some(described) => described,
                        None => {
                            facade.get().mark_unknown(format!(
                                "query columns of expression source {source_name} (planned as declared)"
                            ));
                            native_field(entry, "columns")?
                                .try_iter()?
                                .map(|column| native_field(&column?, "name")?.extract())
                                .collect::<PyResult<_>>()?
                        }
                    },
                    Err(_) => {
                        let names = call_with(
                            &native_field(adapter, "query_column_names")?,
                            &[("connection", connection.clone()), ("sql", expression)],
                        )?;
                        let described: Vec<String> = names
                            .try_iter()?
                            .map(|name| name?.extract())
                            .collect::<PyResult<_>>()?;
                        capture_source_columns(&source_name, &sql, &described);
                        described
                    }
                };
                let mut columns: Vec<_> = Vec::new();
                for name in &described {
                    columns.push(call_with(
                        &column_info,
                        &[("name", text(py, name)), ("type", text(py, ""))],
                    )?);
                }
                result.set_item(source, tuple(py, columns)?)?;
            }
            continue;
        }
        let schema = native_field(entry, "schema")?;
        if schema.is_none() {
            continue;
        }
        let key: String = native_field(entry, "database")?
            .extract::<Option<String>>()?
            .unwrap_or_default();
        let schema: String = schema.extract()?;
        match groups.iter_mut().find(|(group, _)| *group == key) {
            Some((_, schemas)) => {
                schemas.insert(schema);
            }
            None => groups.push((key, BTreeSet::from([schema]))),
        }
    }
    let lookup = host::entry(py, RELATION_LOOKUP)?.getattr("key")?;
    for (key, schemas) in groups {
        let database = (!key.is_empty()).then_some(key.as_str());
        let database_value = match database {
            Some(database) => text(py, database),
            None => py.None().into_bound(py),
        };
        let mut scoped: Vec<(String, Bound<'py, PyAny>, Bound<'py, PyAny>)> = Vec::new();
        for entry in &entries {
            if !native_field(entry, "expression")?.is_none()
                || database_of(entry)?.as_deref() != database
            {
                continue;
            }
            let schema: Option<String> = native_field(entry, "schema")?.extract()?;
            if !schema.is_some_and(|schema| schemas.contains(&schema)) {
                continue;
            }
            let identity = call_with(
                &lookup,
                &[
                    ("database", database_value.clone()),
                    ("schema", native_field(entry, "schema")?),
                    ("name", text(py, &table_name(entry)?)),
                ],
            )?;
            let entry_name: String = native_field(entry, "name")?.extract()?;
            match scoped.iter_mut().find(|(name, _, _)| *name == entry_name) {
                Some(existing) => existing.2 = identity,
                None => scoped.push((entry_name, native_field(entry, "name")?, identity)),
            }
        }
        let relations = list_relations(
            (adapter, connection),
            &database_value,
            names_tuple(py, &schemas)?,
            match name_filter(&entries, database, &schemas)? {
                Some(names) => names_tuple(py, &names)?,
                None => py.None().into_bound(py),
            },
        )?;
        let relations: Vec<Bound<'py, PyAny>> = relations.try_iter()?.collect::<PyResult<_>>()?;
        let identities = frozenset(
            py,
            &relations
                .iter()
                .map(|relation| native_field(relation, "identity"))
                .collect::<PyResult<Vec<_>>>()?,
        )?;
        for (text_name, name, identity) in &scoped {
            if identities.contains(identity)? && listed_names.insert(text_name.clone()) {
                listed.push(name.clone());
            }
        }
        let wanted = frozenset(
            py,
            &scoped
                .iter()
                .map(|(_, _, identity)| identity.clone())
                .collect::<Vec<_>>(),
        )?;
        let mut chosen: Vec<_> = Vec::new();
        for relation in &relations {
            if wanted.contains(native_field(relation, "identity")?)? {
                chosen.push(relation.clone());
            }
        }
        let all = columns_for((adapter, connection), tuple(py, chosen)?)?;
        for (_, name, identity) in &scoped {
            if all.contains(identity)? {
                result.set_item(name, all.get_item(identity)?)?;
            }
        }
    }
    Ok((result, frozenset(py, &listed)?))
}

pub(crate) fn gather_source_inspection<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let (columns, listed) = source_inspection(project, (adapter, connection), source_entries)?;
    Ok(PyTuple::new(project.py(), [columns.into_any(), listed])?.into_any())
}

#[pyfunction]
#[pyo3(signature = (*, project, adapter, connection, source_entries=None))]
pub(crate) fn gather_source_columns<'py>(
    project: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    source_entries: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    Ok(source_inspection(project, (adapter, connection), source_entries)?.0)
}
