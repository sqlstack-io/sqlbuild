//! Selected-scope validation that external source tables exist in the warehouse.

use crate::planner_warehouse::constants::MISSING_SOURCE_TABLE_CODE;
use crate::planner_warehouse::constants::WILDCARD_TABLE_CHARACTER;
use crate::planner_warehouse::models::PlannerReads;
use crate::planner_warehouse::models::Read;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyDict as KeywordDict;
use pyo3::types::PyString;
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_analysis::planner_scope::constants::SOURCE;
use sqlbuild_analysis::planner_warehouse::main::missing_source_tables_help::missing_source_tables_help;
use sqlbuild_analysis::planner_warehouse::main::missing_source_tables_message::missing_source_tables_message;
use sqlbuild_analysis::planner_warehouse::models::MissingSourceTable;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::models::Dialect;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_front::discovery::main::project::yaml_entry_line::yaml_entry_line;
use sqlbuild_python_project::planner_phases::main::planner_error::planner_error;
use sqlbuild_python_project::planner_scope::main::scope_data::scope_data;
use sqlbuild_python_project::references::main::render_source_relation::render_source_relation;
use sqlbuild_python_project::resource_views::main::source_reads::source_entry::source_entry;
use sqlbuild_python_project::resource_views::main::source_reads::source_file::source_file;
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
        && native_field(entry, "expression")?.is_none()
        && !native_field(entry, "schema")?.is_none()
        && !table_name(entry)?.contains(WILDCARD_TABLE_CHARACTER))
}

fn read_relation<'py>(
    dialect: &Bound<'py, Dialect>,
    entry: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyString::new(
        dialect.py(),
        &render_source_relation(entry, Some(dialect.as_any()))?,
    )
    .into_any())
}

fn declaration_location(source: &Bound<'_, PyAny>) -> PyResult<String> {
    let file = source_file(source)?;
    let path: String = native_field(&file, "relative_path")?
        .call_method0("as_posix")?
        .extract()?;
    let line = yaml_entry_line(
        &native_field(&file, "contents")?.extract::<String>()?,
        &sqlbuild_python_project::resource_views::main::source_reads::name::name(source)?
            .extract::<String>()?,
    );
    Ok(match line {
        None => path,
        Some(line) => format!("{path}:{line}"),
    })
}

/// Raise S405 when a selected resource reads an external source table that does not exist.
pub(crate) fn check_selected_source_tables_exist<'py>(
    project: &Bound<'py, PyAny>,
    (dialect, reads): (&Bound<'py, Dialect>, &Bound<'py, PlannerReads>),
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py = project.py();
    let readers = selected_source_readers(scope)?;
    let read_map = native_field(relations, "source_read_map")?;
    let listed = native_field(relations, "listed_source_names")?;
    let mut unlisted: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>, Vec<String>)> = Vec::new();
    for source in sqlbuild_python_project::compiled_views::main::project::sources::sources(project)?
        .try_iter()?
    {
        let source = source?;
        let name: String =
            sqlbuild_python_project::resource_views::main::source_reads::name::name(&source)?
                .extract()?;
        let Some(source_readers) = readers.get(&name).filter(|readers| !readers.is_empty()) else {
            continue;
        };
        let declared = source_entry(&source)?;
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
    let relations = || -> PyResult<Vec<Bound<'py, PyAny>>> {
        unlisted
            .iter()
            .map(|(_, entry, _)| read_relation(dialect, entry))
            .collect()
    };
    let Read::Known(probes) = reads.get().reads().read_access(py, &relations)? else {
        return Ok(());
    };
    let mut missing: Vec<_> = Vec::new();
    let mut entries: Vec<_> = Vec::new();
    for ((source, entry, read_by), probe) in unlisted.into_iter().zip(probes) {
        let status = native_field(&probe, "status")?;
        if status.eq("readable")? {
            continue;
        }
        let role = native_field(&probe, "role")?;
        missing.push(MissingSourceTable {
            source_name: sqlbuild_python_project::resource_views::main::source_reads::name::name(
                &source,
            )?
            .extract()?,
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
        item.relation = read_relation(dialect, entry)?.str()?.extract()?;
    }
    let message = missing_source_tables_message(&missing);
    let sources = PyDict::new(py);
    for source in sqlbuild_python_project::compiled_views::main::project::sources::sources(project)?
        .try_iter()?
    {
        let source = source?;
        sources.set_item(
            sqlbuild_python_project::resource_views::main::source_reads::name::name(&source)?,
            source,
        )?;
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
        (&["project", "dialect", "reads", "scope", "relations"], &[]),
    )?;
    let project = &args.value("project")?;
    let dialect = &Dialect::passed(args.value("dialect")?)?;
    let reads = &PlannerReads::passed(args.value("reads")?)?;
    let scope = &args.value("scope")?;
    let relations = &args.value("relations")?;

    check_selected_source_tables_exist(project, (dialect, reads), scope, relations)
}
