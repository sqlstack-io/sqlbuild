//! Static plans for commands that do not build models, and direct seed identity reads.

use crate::planner_entries::_helpers::entries::values::{
    boolean, none, planner_model, planner_type,
};
use crate::planner_entries::_helpers::plans::plan_output::{
    function_entry, selected_test_entries, test_selected,
};
use crate::planner_identity::main::build_compiled_function_fingerprint_sql::build_compiled_function_fingerprint_sql;
use crate::planner_identity::main::build_seed_identity::build_seed_identity;
use crate::planner_strategy::main::model_entries::extract_seed_columns::extract_seed_columns;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{
    PyDict, PyDictMethods, PyFrozenSet, PyList, PyListMethods, PySet, PySetMethods, PyString,
    PyTuple,
};
use sqlbuild_analysis::fingerprint_storage::constants::NODE_TYPE_SEED;
use sqlbuild_core::constants::FINGERPRINT_TABLE_NAME;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::project::seeds::seeds;
use sqlbuild_python_project::compiled_views::main::project::sql_tests::sql_tests;
use sqlbuild_python_project::fingerprints::main::read_latest_fingerprints::read_latest_fingerprints;
use sqlbuild_python_project::planner_phases::main::object::object;
use sqlbuild_python_project::resource_views::main::function_reads::deps::deps;
use sqlbuild_python_project::resource_views::main::seed_reads::schema_entry::schema_entry;
use sqlbuild_python_project::resource_views::main::seed_reads::seed_file::seed_file;
use std::collections::{BTreeMap, BTreeSet};

/// Optional fields of a static command plan; absent ones take `PlanOutput` defaults.
#[derive(Default)]
pub(crate) struct StaticPlanParts<'py> {
    pub(crate) execution_order: Option<Bound<'py, PyAny>>,
    pub(crate) fields: Vec<(&'static str, Bound<'py, PyAny>)>,
}

pub(crate) fn base_plan_output<'py>(
    project: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
    parts: StaticPlanParts<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let selected_keys = native_field(scope, "selected_keys")?;
    let execution_order = match parts.execution_order {
        Some(order) => order,
        None => {
            let mut order: Vec<Bound<'py, PyAny>> = Vec::new();
            for key in native_field(scope, "execution_order")?.try_iter()? {
                let key = key?;
                if selected_keys.contains(&key)? {
                    order.push(key);
                }
            }
            PyTuple::new(py, order)?.into_any()
        }
    };
    let mut fields = vec![("execution_order", execution_order)];
    fields.extend(parts.fields);
    fields.extend([
        ("selected_keys", selected_keys),
        ("upstream_deps", native_field(scope, "upstream_deps")?),
        ("downstream_deps", native_field(scope, "downstream_deps")?),
        (
            "model_locations",
            native_field(relations, "model_locations")?,
        ),
        ("seed_locations", native_field(relations, "seed_locations")?),
        (
            "function_locations",
            native_field(relations, "function_locations")?,
        ),
        ("source_map", native_field(relations, "source_map")?),
        (
            "source_read_map",
            native_field(relations, "source_read_map")?,
        ),
        (
            "python_source_read_map",
            native_field(relations, "python_source_read_map")?,
        ),
        ("hook_functions", native_field(project, "hook_functions")?),
        (
            "enforce_explicit_references",
            native_field(project, "enforce_explicit_references")?,
        ),
    ]);
    planner_model(py, RecordId::PlanOutput, &fields)
}

/// The tests' direct function dependencies and every project function those call.
fn called_function_keys<'py>(
    project: &Bound<'py, PyAny>,
    direct: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PySet>> {
    let py: Python<'py> = project.py();
    let functions: Bound<'py, PyDict> = PyDict::new(py);
    for function in
        sqlbuild_python_project::compiled_views::main::project::functions::functions(project)?
            .try_iter()?
    {
        let function = function?;
        functions.set_item(
            sqlbuild_python_project::resource_views::main::function_reads::key::key(&function)?,
            function,
        )?;
    }
    let called: Bound<'py, PySet> = PySet::empty(py)?;
    let mut pending: Vec<Bound<'py, PyAny>> = direct;
    while let Some(key) = pending.pop() {
        if called.contains(&key)? {
            continue;
        }
        called.add(&key)?;
        let Some(function) = functions.get_item(&key)? else {
            continue;
        };
        for dependency in deps(&function)?.try_iter()? {
            let dependency = dependency?;
            if functions.contains(&dependency)? {
                pending.push(dependency);
            }
        }
    }
    Ok(called)
}

/// Project selected SQL tests from canonical static state.
#[pyfunction]
#[pyo3(signature = (**kwargs))]
pub(crate) fn build_test_command_plan<'py>(
    py: Python<'py>,
    kwargs: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let args = KeywordArguments::read(
        py,
        "build_test_command_plan",
        kwargs,
        (
            &["project", "adapter", "scope", "relations"],
            &["case_name"],
        ),
    )?;
    let project = args.value("project")?;
    let adapter = args.value("adapter")?;
    let scope = args.value("scope")?;
    let relations = args.value("relations")?;
    let selected_keys = native_field(&scope, "selected_keys")?;
    let selection = native_field(&scope, "sql_test_selection")?;
    let selected = selected_test_entries(
        (&project, &adapter),
        &selected_keys,
        &selection,
        &args.value("case_name")?,
    )?;
    let (entries, warnings) = (selected.entries, selected.warnings);
    let mut case_names: BTreeSet<String> = BTreeSet::new();
    for test in sql_tests(&project)?.try_iter()? {
        let test = test?;
        let case_name = native_field(&test, "case_name")?;
        if !case_name.is_none() && test_selected(&test, &selected_keys, &selection)? {
            case_names.insert(case_name.extract()?);
        }
    }
    let mut test_keys: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut function_keys: Vec<Bound<'py, PyAny>> = Vec::new();
    for entry in &entries {
        test_keys.push(native_field(entry, "key")?);
        for key in native_field(entry, "function_deps")?.try_iter()? {
            function_keys.push(key?);
        }
    }
    let function_keys = called_function_keys(&project, function_keys)?;
    let mut functions: Vec<Bound<'py, PyAny>> = Vec::new();
    for function in
        sqlbuild_python_project::compiled_views::main::project::functions::functions(&project)?
            .try_iter()?
    {
        let function = function?;
        if !function_keys.contains(
            sqlbuild_python_project::resource_views::main::function_reads::key::key(&function)?,
        )? {
            continue;
        }
        let fingerprint_sql = object(py, build_compiled_function_fingerprint_sql(&function)?)?;
        functions.push(function_entry(
            &function,
            &adapter,
            &relations,
            (
                fingerprint_sql,
                none(py),
                planner_type(py, RecordId::PlanReason, "NO_CHANGE")?,
            ),
        )?);
    }
    let scoped_keys = selected_keys.call_method1("__or__", (PyFrozenSet::new(py, test_keys)?,))?;
    let mut order: Vec<Bound<'py, PyAny>> = Vec::new();
    for key in native_field(&scope, "execution_order")?.try_iter()? {
        let key = key?;
        if scoped_keys.contains(&key)? {
            order.push(key);
        }
    }
    base_plan_output(
        &project,
        &scope,
        &relations,
        StaticPlanParts {
            execution_order: Some(PyTuple::new(py, order)?.into_any()),
            fields: vec![
                ("test_entries", PyTuple::new(py, entries)?.into_any()),
                (
                    "available_test_case_names",
                    PyTuple::new(py, case_names)?.into_any(),
                ),
                ("function_entries", PyTuple::new(py, functions)?.into_any()),
                ("warnings", PyTuple::new(py, warnings)?.into_any()),
            ],
        },
    )
}

/// Project selected direct seed work and canonical identities.
#[pyfunction]
#[pyo3(signature = (*, project, scope, relations, fingerprints = None))]
pub(crate) fn build_seed_command_plan<'py>(
    project: &Bound<'py, PyAny>,
    scope: &Bound<'py, PyAny>,
    relations: &Bound<'py, PyAny>,
    fingerprints: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py = project.py();
    let existing = match fingerprints {
        Some(fingerprints) if fingerprints.is_truthy()? => fingerprints,
        _ => PyDict::new(py).into_any(),
    };
    let selected_keys = native_field(scope, "selected_keys")?;
    let mut entries: Vec<Bound<'py, PyAny>> = Vec::new();
    for seed in seeds(project)?.try_iter()? {
        let seed = seed?;
        if !selected_keys
            .contains(sqlbuild_python_project::resource_views::main::seed_reads::key::key(&seed)?)?
        {
            continue;
        }
        let identity = object(py, build_seed_identity(&seed)?)?;
        let (version_hash, metadata_json) = (identity.get_item(0)?, identity.get_item(1)?);
        let previous = mapping_get(
            &existing,
            sqlbuild_python_project::resource_views::main::seed_reads::name::name(&seed)?,
        )?;
        let reason = if previous.is_none() {
            "FIRST_RUN"
        } else if native_field(&previous, "version_hash")?.ne(&version_hash)? {
            "CONFIG_CHANGED"
        } else {
            "NO_CHANGE"
        };
        entries.push(planner_model(
            py,
            RecordId::SeedPlanEntry,
            &[
                ("key", sqlbuild_python_project::resource_views::main::seed_reads::key::key(&seed)?),
                ("name", sqlbuild_python_project::resource_views::main::seed_reads::name::name(&seed)?),
                ("destination", sqlbuild_python_project::resource_views::main::seed_reads::destination::destination(&seed)?),
                (
                    "file_path",
                    native_field(&seed_file(&seed)?, "file_path")?,
                ),
                ("columns", object(py, extract_seed_columns(&seed)?)?),
                (
                    "csv_settings",
                    native_field(&schema_entry(&seed)?, "csv_settings")?,
                ),
                ("fingerprint_definition", metadata_json.clone()),
                ("fingerprint_version_hash", version_hash),
                ("fingerprint_metadata_json", metadata_json),
                ("reason", planner_type(py, RecordId::PlanReason, reason)?),
            ],
        )?);
    }
    base_plan_output(
        project,
        scope,
        relations,
        StaticPlanParts {
            execution_order: None,
            fields: vec![("seed_entries", PyTuple::new(py, entries)?.into_any())],
        },
    )
}

/// A schema target's sort key: the database with `None` first as an empty name, then the schema.
type Target = (Option<String>, String);

/// Read only fingerprint state required by selected direct seeds.
#[pyfunction]
#[pyo3(signature = (*, adapter, connection, seeds))]
pub(crate) fn read_selected_seed_fingerprints<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    seeds: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py = adapter.py();
    let table_name: &str = FINGERPRINT_TABLE_NAME;
    let mut names_by_target: BTreeMap<(String, String), (Target, BTreeSet<String>)> =
        BTreeMap::new();
    for seed in seeds.try_iter()? {
        let seed = seed?;
        let destination =
            sqlbuild_python_project::resource_views::main::seed_reads::destination::destination(
                &seed,
            )?;
        let schema = native_field(&destination, "schema")?;
        if schema.is_none() {
            continue;
        }
        let database: Option<String> = native_field(&destination, "database")?.extract()?;
        let schema: String = schema.extract()?;
        names_by_target
            .entry((database.clone().unwrap_or_default(), schema.clone()))
            .or_insert_with(|| ((database, schema), BTreeSet::new()))
            .1
            .insert(
                sqlbuild_python_project::resource_views::main::seed_reads::name::name(&seed)?
                    .extract()?,
            );
    }
    let fingerprints = PyDict::new(py);
    let mut relations_by_target: Vec<Bound<'py, PyAny>> = Vec::new();
    for ((database, schema), _) in names_by_target.values() {
        let names = PyTuple::new(py, [table_name])?;
        let schemas = PyTuple::new(py, [schema.as_str()])?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("connection", connection)?;
        kwargs.set_item("database", database)?;
        kwargs.set_item("schemas", schemas)?;
        kwargs.set_item("names", names)?;
        relations_by_target.push(native_field(adapter, "list_relations")?.call((), Some(&kwargs))?);
    }
    for (((database, schema), names), relations) in
        names_by_target.values().zip(relations_by_target)
    {
        let mut table_exists = false;
        for relation in relations.try_iter()? {
            let name: String = native_field(&relation?, "name")?.extract()?;
            if name.to_lowercase() == table_name.to_lowercase() {
                table_exists = true;
                break;
            }
        }
        let node_names = PyTuple::new(py, names.iter())?;
        let latest = read_latest_fingerprints(Some(&keywords(
            py,
            &[
                ("connection", connection.clone()),
                ("execute", native_field(adapter, "execute")?),
                ("table_exists", boolean(py, table_exists)?),
                (
                    "database",
                    database
                        .as_deref()
                        .map_or_else(|| none(py), |name| PyString::new(py, name).into_any()),
                ),
                ("schema", PyString::new(py, schema).into_any()),
                (
                    "render_qualified_name",
                    native_field(adapter, "render_qualified_name")?,
                ),
                (
                    "render_read_latest_sql",
                    native_field(adapter, "render_read_latest_fingerprints_sql")?,
                ),
                ("node_names", node_names.into_any()),
            ],
        )?))?;
        let items = PyList::new(
            py,
            native_field(&latest, "fingerprints")?
                .call_method0("items")?
                .try_iter()?
                .collect::<PyResult<Vec<_>>>()?,
        )?;
        for item in items.iter() {
            let (name, fingerprint) = (item.get_item(0)?, item.get_item(1)?);
            if native_field(&fingerprint, "node_type")?.eq(NODE_TYPE_SEED)?
                && names.contains(&name.extract::<String>()?)
            {
                fingerprints.set_item(name, fingerprint)?;
            }
        }
    }
    Ok(fingerprints)
}
