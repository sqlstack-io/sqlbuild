//! Selected-scope validation that external source tables exist in the warehouse.

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::boundary::python_calls::{call_with, imported, keywords};
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::compiled_views::source_reads;
use crate::bindings::_helpers::discovery::locations::yaml_entry_line;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::planner_phases::objects::{planner_error, tuple};
use crate::bindings::_helpers::planner_scope::retained::scope_data;
use crate::bindings::_helpers::planner_warehouse::constants::{
    MISSING_SOURCE_TABLE_CODE, RUN_BOUNDED, WILDCARD_TABLE_CHARACTER,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get_or;
use crate::bindings::_helpers::references::source_relations::render_source_relation;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict as KeywordDict;
use pyo3::types::PyString;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::planner_scope::constants::SOURCE;
use sqlbuild_analysis::planner_warehouse::main::missing_source_tables_help::missing_source_tables_help;
use sqlbuild_analysis::planner_warehouse::main::missing_source_tables_message::missing_source_tables_message;
use sqlbuild_analysis::planner_warehouse::models::MissingSourceTable;
use std::collections::HashMap;

fn selected_source_readers(scope: &Bound<'_, PyAny>) -> PyResult<HashMap<String, Vec<String>>> {
    let mut readers: HashMap<String, Vec<String>> = HashMap::new();
    let scope = scope_data(scope)?;
    let deps: HashMap<_, _> = scope
        .upstream
        .iter()
        .map(|(key, deps)| (key, deps))
        .collect();
    for key in &scope.selected {
        if key.0 == SOURCE {
            continue;
        }
        let reader = &key.1;
        for dep in deps.get(key).into_iter().flat_map(|deps| deps.iter()) {
            if dep.0 == SOURCE {
                readers
                    .entry(dep.1.clone())
                    .or_default()
                    .push(reader.clone());
            }
        }
    }
    Ok(readers)
}

fn table_name(entry: &Bound<'_, PyAny>) -> PyResult<String> {
    let table = native_field(entry, "table")?;
    if table.is_none() {
        native_field(entry, "name")?.extract()
    } else {
        table.extract()
    }
}

fn external_table(entry: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(!native_field(entry, "managed")?.is_truthy()?
        && native_field(entry, "loader")?.is_none()
        && native_field(entry, "integration_loader")?.is_none()
        && native_field(entry, "expression")?.is_none()
        && !native_field(entry, "schema")?.is_none()
        && !table_name(entry)?.contains(WILDCARD_TABLE_CHARACTER))
}

fn read_relation<'py>(
    adapter: &Bound<'py, PyAny>,
    entry: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyString::new(adapter.py(), &render_source_relation(entry, Some(adapter))?).into_any())
}

fn declaration_location(source: &Bound<'_, PyAny>) -> PyResult<String> {
    let file = source_reads::source_file(source)?;
    let path: String = native_field(&file, "relative_path")?
        .call_method0("as_posix")?
        .extract()?;
    let line = yaml_entry_line(
        &native_field(&file, "contents")?.extract::<String>()?,
        &source_reads::name(source)?.extract::<String>()?,
    );
    Ok(match line {
        None => path,
        Some(line) => format!("{path}:{line}"),
    })
}

/// Raise S405 when a selected resource reads an external source table that does not exist.
pub(crate) fn check_selected_source_tables_exist<'py>(
    project: &Bound<'py, PyAny>,
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py = project.py();
    let readers = selected_source_readers(scope)?;
    let read_map = native_field(relations, "source_read_map")?;
    let listed = native_field(relations, "listed_source_names")?;
    let mut unlisted: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>, Vec<String>)> = Vec::new();
    for source in project_reads::sources(project)?.try_iter()? {
        let source = source?;
        let name: String = source_reads::name(&source)?.extract()?;
        let Some(source_readers) = readers.get(&name).filter(|readers| !readers.is_empty()) else {
            continue;
        };
        let declared = source_reads::source_entry(&source)?;
        let entry = mapping_get_or(&read_map, &name, &declared)?;
        if !external_table(&declared)? || !external_table(&entry)? {
            continue;
        }
        if listed.contains(&name)? {
            continue;
        }
        let mut sorted = source_readers.clone();
        sorted.sort();
        unlisted.push((source, entry, sorted));
    }
    if unlisted.is_empty() {
        return Ok(());
    }
    let probe = native_field(adapter, "probe_relation_read")?;
    let mut tasks: Vec<_> = Vec::new();
    for (_, entry, _) in &unlisted {
        let fields = keywords(
            py,
            &[
                ("connection", connection.clone()),
                ("relation", read_relation(adapter, entry)?),
            ],
        )?;
        tasks.push(imported(py, "functools", "partial")?.call((&probe,), Some(&fields))?);
    }
    let concurrency: i64 = imported(py, "builtins", "int")?
        .call1((native_field(adapter, "metadata_inspection_concurrency")?,))?
        .extract()?;
    let probes = call_with(
        &host::entry(py, RUN_BOUNDED)?,
        &[
            ("tasks", tuple(py, tasks)?),
            (
                "concurrency",
                concurrency.max(1).into_pyobject(py)?.into_any(),
            ),
        ],
    )?;
    let mut missing: Vec<_> = Vec::new();
    let mut entries: Vec<_> = Vec::new();
    for ((source, entry, read_by), probe) in unlisted.into_iter().zip(probes.try_iter()?) {
        let probe = probe?;
        let status = native_field(&probe, "status")?;
        if status.eq("readable")? {
            continue;
        }
        let role = native_field(&probe, "role")?;
        missing.push(MissingSourceTable {
            source_name: source_reads::name(&source)?.extract()?,
            relation: String::new(),
            read_by,
            unreadable: status.eq("missing_or_unreadable")?,
            role: if role.is_none() {
                None
            } else {
                Some(role.str()?.extract()?)
            },
            location: String::new(),
        });
        entries.push(entry);
    }
    if missing.is_empty() {
        return Ok(());
    }
    for (item, entry) in missing.iter_mut().zip(&entries).take(10) {
        item.relation = read_relation(adapter, entry)?.str()?.extract()?;
    }
    let message = missing_source_tables_message(&missing);
    let sources = PyDict::new(py);
    for source in project_reads::sources(project)?.try_iter()? {
        let source = source?;
        sources.set_item(source_reads::name(&source)?, source)?;
    }
    for item in missing.iter_mut().take(10) {
        if let Some(source) = sources.get_item(&item.source_name)? {
            item.location = declaration_location(&source)?;
        }
    }
    let help = missing_source_tables_help(&missing);
    Err(planner_error(
        py,
        &message,
        (Some(MISSING_SOURCE_TABLE_CODE), Some(&help)),
    ))
}

#[pyfunction]
#[pyo3(name = "check_selected_source_tables_exist")]
#[pyo3(signature = (**options))]
pub(crate) fn check_selected_source_tables_exist_binding<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<()> {
    let args = KeywordArguments::read(
        py,
        "check_selected_source_tables_exist",
        options,
        (
            &["project", "adapter", "connection", "scope", "relations"],
            &[],
        ),
    )?;
    let project = &args.value("project")?;
    let adapter = &args.value("adapter")?;
    let connection = &args.value("connection")?;
    let scope = &args.value("scope")?;
    let relations = &args.value("relations")?;

    check_selected_source_tables_exist(project, (adapter, connection), scope, relations)
}
