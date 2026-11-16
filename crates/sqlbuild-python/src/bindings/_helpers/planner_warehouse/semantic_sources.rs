//! Complete missing source metadata in bounded, database-grouped batches.

use crate::bindings::_helpers::compiled_views::model_reads;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::model_config::typed_config::{config_node_of, model_values};
use crate::bindings::_helpers::planner_phases::objects::{text, tuple};
use crate::bindings::_helpers::planner_warehouse::constants::ACTIVE_CATALOG;
use crate::bindings::_helpers::planner_warehouse::snapshot::{columns_for, list_relations};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict as KeywordDict;
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use std::collections::BTreeSet;

const SQL_ANALYSIS_KEY: &str = "sql_analysis";

fn casefold(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.call_method0("casefold")?.extract()
}

fn wanted_name<'py>(entry: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let table = native_field(entry, "table")?;
    if table.is_truthy()? {
        Ok(table)
    } else {
        native_field(entry, "name")
    }
}

fn matches(entry: &Bound<'_, PyAny>, relation: &Bound<'_, PyAny>) -> PyResult<bool> {
    if casefold(&native_field(relation, "name")?)? != casefold(&wanted_name(entry)?)? {
        return Ok(false);
    }
    let schema = native_field(entry, "schema")?;
    if schema.is_none() {
        return Ok(true);
    }
    let relation_schema = native_field(relation, "schema")?;
    let relation_schema = if relation_schema.is_truthy()? {
        relation_schema
    } else {
        text(relation.py(), "")
    };
    Ok(casefold(&relation_schema)? == casefold(&schema)?)
}

fn sorted_texts<'py>(py: Python<'py>, values: BTreeSet<String>) -> PyResult<Bound<'py, PyAny>> {
    tuple(py, values.iter().map(|value| text(py, value)).collect())
}

fn source_candidates<'py>(
    (adapter, connection): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    database: &Bound<'py, PyAny>,
    entries: &[Bound<'py, PyAny>],
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = adapter.py();
    let mut pending: Vec<Bound<'py, PyAny>> = entries.to_vec();
    let mut relations: Vec<Bound<'py, PyAny>> = Vec::new();
    let catalog = call_with(
        &host_entry(py, ACTIVE_CATALOG)?,
        &[
            ("adapter", adapter.clone()),
            ("connection", connection.clone()),
        ],
    )?;
    let mut scoped: Vec<_> = Vec::new();
    for entry in entries {
        if !native_field(entry, "schema")?.is_none() {
            scoped.push(entry.clone());
        }
    }
    if !catalog.is_none()
        && native_field(&catalog, "schema_scoped")?.is_truthy()?
        && !scoped.is_empty()
    {
        let mut schemas: BTreeSet<_> = BTreeSet::new();
        let mut names: BTreeSet<_> = BTreeSet::new();
        for entry in &scoped {
            schemas.insert(
                native_field(entry, "schema")?
                    .extract::<Option<String>>()?
                    .unwrap_or_default(),
            );
            names.insert(wanted_name(entry)?.extract::<String>()?);
        }
        let listed = list_relations(
            (adapter, connection),
            database,
            sorted_texts(py, schemas)?,
            sorted_texts(py, names)?,
        )?;
        for relation in listed.try_iter()? {
            relations.push(relation?);
        }
        let mut remaining: Vec<_> = Vec::new();
        for entry in entries {
            let mut found = false;
            for relation in &relations {
                if matches(entry, relation)? {
                    found = true;
                    break;
                }
            }
            if !found {
                remaining.push(entry.clone());
            }
        }
        pending = remaining;
    }
    if !pending.is_empty() {
        let mut names: BTreeSet<_> = BTreeSet::new();
        for entry in &pending {
            names.insert(wanted_name(entry)?.extract::<String>()?);
        }
        let listed = list_relations(
            (adapter, connection),
            database,
            py.None().into_bound(py),
            sorted_texts(py, names)?,
        )?;
        for relation in listed.try_iter()? {
            relations.push(relation?);
        }
    }
    let unique = PyDict::new(py);
    for relation in relations {
        let identity = native_field(&relation, "identity")?;
        if !unique.contains(&identity)? {
            unique.set_item(identity, relation)?;
        }
    }
    unique.values().try_iter()?.collect()
}

/// Source relation candidates by name; schema listings first answer schema-scoped catalogs.
#[pyfunction]
#[pyo3(signature = (*, adapter, connection, database, entries))]
pub(crate) fn list_source_candidates<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    database: &Bound<'py, PyAny>,
    entries: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let entries: Vec<Bound<'py, PyAny>> = entries.try_iter()?.collect::<PyResult<_>>()?;
    tuple(
        adapter.py(),
        source_candidates((adapter, connection), database, &entries)?,
    )
}

/// Reuse collected columns and inspect only selected, unresolved table sources.
#[pyfunction]
#[pyo3(name = "get_semantic_source_columns")]
#[pyo3(signature = (**options))]
pub(crate) fn semantic_source_columns<'py>(
    py: pyo3::Python<'py>,
    options: Option<Bound<'py, KeywordDict>>,
) -> PyResult<Bound<'py, PyDict>> {
    let args = KeywordArguments::read(
        py,
        "get_semantic_source_columns",
        options,
        (
            &[
                "project",
                "adapter",
                "connection",
                "source_read_map",
                "columns",
                "selected_keys",
            ],
            &[],
        ),
    )?;
    let project = &args.value("project")?;
    let adapter = &args.value("adapter")?;
    let connection = &args.value("connection")?;
    let source_read_map = &args.value("source_read_map")?;
    let columns = args.value("columns")?.downcast_into::<PyDict>()?;
    let columns = &columns;
    let selected_keys = &args.value("selected_keys")?;

    let py = project.py();
    if !native_field(&native_field(project, "settings")?, "sql_analysis")?.is_truthy()?
        || connection.is_none()
    {
        return Ok(columns.clone());
    }
    let mut required: BTreeSet<String> = BTreeSet::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model = model?;
        let analysis_off: bool = matches!(
            *config_node_of(&model_values(&model)?, SQL_ANALYSIS_KEY)?,
            ConfigNode::Bool(false)
        );
        if !selected_keys.contains(model_reads::model_key(&model)?)? || analysis_off {
            continue;
        }
        for reference in model_reads::references(&model)?.try_iter()? {
            let reference = reference?;
            if native_field(&reference, "ref_kind")?.eq("source")? {
                required.insert(native_field(&reference, "ref_name")?.extract()?);
            }
        }
    }
    let mut groups: Vec<(Bound<'py, PyAny>, Vec<Bound<'py, PyAny>>)> = Vec::new();
    for name in required {
        if columns.contains(&name)? {
            continue;
        }
        let entry = mapping_get(source_read_map, &name)?;
        if entry.is_none()
            || !native_field(&entry, "expression")?.is_none()
            || native_field(&entry, "contract")?.eq("enforced")?
        {
            continue;
        }
        let database = native_field(&entry, "database")?;
        let mut placed = false;
        for (group, entries) in groups.iter_mut() {
            if group.eq(&database)? {
                entries.push(entry.clone());
                placed = true;
                break;
            }
        }
        if !placed {
            groups.push((database, vec![entry]));
        }
    }
    let result = columns.copy()?;
    for (database, entries) in groups {
        let relations = source_candidates((adapter, connection), &database, &entries)?;
        let mut chosen: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = Vec::new();
        for entry in &entries {
            let mut candidates: Vec<_> = Vec::new();
            for relation in &relations {
                if matches(entry, relation)? {
                    candidates.push(relation.clone());
                }
            }
            if candidates.len() == 1 {
                let name = native_field(entry, "name")?;
                match chosen
                    .iter_mut()
                    .find(|(chosen_name, _)| chosen_name.eq(&name).unwrap_or(false))
                {
                    Some(existing) => existing.1 = candidates.remove(0),
                    None => chosen.push((name, candidates.remove(0))),
                }
            }
        }
        let inspected = if chosen.is_empty() {
            PyDict::new(py).into_any()
        } else {
            let unique = PyDict::new(py);
            for (_, relation) in &chosen {
                unique.set_item(relation, py.None())?;
            }
            columns_for(
                (adapter, connection),
                PyTuple::new(py, unique.keys())?.into_any(),
            )?
        };
        for (name, relation) in chosen {
            let lower = |value: Bound<'py, PyAny>| -> PyResult<Bound<'py, PyAny>> {
                if value.is_truthy()? {
                    value.call_method0("lower")
                } else {
                    Ok(py.None().into_bound(py))
                }
            };
            let identity = PyTuple::new(
                py,
                [
                    lower(native_field(&relation, "database")?)?,
                    lower(native_field(&relation, "schema")?)?,
                    native_field(&relation, "name")?.call_method0("lower")?,
                ],
            )?;
            if inspected.contains(&identity)? {
                result.set_item(name, inspected.get_item(identity)?)?;
            }
        }
    }
    Ok(result)
}
