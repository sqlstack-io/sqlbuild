//! Sources a load fills because SQL reads them: `__source("shop__orders")` with no entry listed.

use std::collections::{BTreeSet, HashSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_python_base::compile_context::main::delta::injection::injection;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_rules::lint_text::main::dependency_sites::dependency_sites;
use sqlbuild_rules::lint_text::main::neutralize_interpolation::neutralize_interpolation;

/// The discovered files whose SQL may read a source.
const SQL_FILE_FIELDS: [&str; 7] = [
    "model_files",
    "sql_function_files",
    "sql_hook_files",
    "test_files",
    "scenario_files",
    "audit_files",
    "macro_files",
];
/// How a source reference's relation identity starts.
const SOURCE_RELATION_PREFIX: &str = "source:";
/// Between a load's name and the table it loads in a source's name.
const LOAD_SEPARATOR: &str = "__";

/// Give each load's sources file an entry for every unlisted `{load}__{table}` source SQL reads.
pub(crate) fn with_implicit_load_sources(
    declarations: &Bound<'_, PyDict>,
    dialect: &str,
) -> PyResult<()> {
    let py: Python<'_> = declarations.py();
    let Some(files) = declarations.get_item("source_files")? else {
        return Ok(());
    };
    let files: Vec<Bound<'_, PyAny>> = files.try_iter()?.collect::<PyResult<_>>()?;
    let mut loads: Vec<String> = Vec::new();
    let mut listed: HashSet<String> = HashSet::new();
    for file in &files {
        let load: Bound<'_, PyAny> = native_field(file, "load")?;
        if !load.is_none() {
            loads.push(native_field(&load, "name")?.extract()?);
        }
        for entry in native_field(file, "source_entries")?.try_iter()? {
            listed.insert(native_field(&entry?, "name")?.extract()?);
        }
    }
    if loads.is_empty() {
        return Ok(());
    }
    let mut referenced: BTreeSet<String> = referenced_sources(declarations, dialect)?;
    if let Some(names) = injection().and_then(|injection| injection.source_names.clone()) {
        referenced.extend(names);
    }
    let mut changed: bool = false;
    let mut rebuilt: Vec<Bound<'_, PyAny>> = Vec::with_capacity(files.len());
    for file in files {
        let load: Bound<'_, PyAny> = native_field(&file, "load")?;
        if load.is_none() {
            rebuilt.push(file);
            continue;
        }
        let load: String = native_field(&load, "name")?.extract()?;
        let added: Vec<&String> = referenced
            .iter()
            .filter(|name| !listed.contains(*name) && filling_load(name, &loads) == Some(&load))
            .collect();
        if added.is_empty() {
            rebuilt.push(file);
            continue;
        }
        let mut entries: Vec<Bound<'_, PyAny>> = native_field(&file, "source_entries")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        for name in added {
            entries.push(implicit_entry(py, name, &load)?);
        }
        rebuilt.push(replaced(
            &file,
            &[("source_entries", PyTuple::new(py, entries)?.into_any())],
        )?);
        changed = true;
    }
    if changed {
        declarations.set_item("source_files", PyTuple::new(py, rebuilt)?)?;
    }
    Ok(())
}

/// Every source name the discovered SQL files read by a literal `__source(...)`.
fn referenced_sources(
    declarations: &Bound<'_, PyDict>,
    dialect: &str,
) -> PyResult<BTreeSet<String>> {
    let mut names: BTreeSet<String> = BTreeSet::new();
    for field in SQL_FILE_FIELDS {
        let Some(files) = declarations.get_item(field)? else {
            continue;
        };
        for file in files.try_iter()? {
            let contents: String = native_field(&file?, "contents")?.extract()?;
            names.extend(source_references(&contents, dialect));
        }
    }
    Ok(names)
}

/// The sources `sql` reads by name, outside comments and strings; unreadable SQL reads none here.
fn source_references(sql: &str, dialect: &str) -> Vec<String> {
    let Ok(neutralized) = neutralize_interpolation(sql, dialect, false) else {
        return Vec::new();
    };
    let Ok(sites) = dependency_sites(&neutralized.sites) else {
        return Vec::new();
    };
    sites
        .relations
        .into_iter()
        .filter_map(|(_, relation)| {
            relation
                .strip_prefix(SOURCE_RELATION_PREFIX)
                .map(str::to_owned)
        })
        .collect()
}

/// The load whose name and `__` begin `name`, the longest when several do.
fn filling_load<'a>(name: &str, loads: &'a [String]) -> Option<&'a String> {
    loads
        .iter()
        .filter(|load| {
            name.strip_prefix(load.as_str())
                .and_then(|rest| rest.strip_prefix(LOAD_SEPARATOR))
                .is_some_and(|table| !table.is_empty())
        })
        .max_by_key(|load| load.len())
}

/// The source entry of `name`, which the load `load` fills and nothing lists.
fn implicit_entry<'py>(py: Python<'py>, name: &str, load: &str) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(
        py,
        RecordId::SourceEntry,
        &[
            ("name", PyString::new(py, name).into_any()),
            ("load", PyString::new(py, load).into_any()),
            (
                "description",
                PyString::new(py, &format!("Loaded by load '{load}'.")).into_any(),
            ),
        ],
    )
}
