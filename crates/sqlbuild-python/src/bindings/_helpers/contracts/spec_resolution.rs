//! Effective target, adapter, rendering, scenario and loader destination resolution.

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_model_config::name_patterns::main::compile_name_pattern::compile_name_pattern;
use sqlbuild_model_config::name_patterns::models::NamePattern;

use crate::bindings::_helpers::contracts::effective_config::{
    effective_config_target_fields, effective_config_target_name,
};
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const TARGET_GROUPS: [(&str, RecordId); 4] = [
    ("clone", RecordId::ClonePolicy),
    ("execution_limits", RecordId::ExecutionLimitsConfig),
    ("diff", RecordId::TargetDiffConfig),
    ("warehouses", RecordId::TargetWarehousesConfig),
];
const SNAPSHOT_LIMITS: [&str; 4] = [
    "max_rows_per_relation",
    "max_total_rows",
    "max_bytes_per_relation",
    "max_total_bytes",
];
const ALL_DIALECTS: &str = "*";
const LOADER_SCHEMA_TABLE_PARTS: usize = 2;
const LOADER_QUALIFIED_TABLE_PARTS: usize = 3;
const NAME_PATTERN_CACHE_LIMIT: usize = 256;

thread_local! {
    static NAME_PATTERNS: RefCell<HashMap<String, NamePattern>> = RefCell::new(HashMap::new());
}

fn spec_error<'py>(py: Python<'py>, message: Bound<'py, PyString>) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::SpecConfig)?.call1((message,))?,
    ))
}

/// Resolve the effective target name.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, selected_target))]
pub(crate) fn resolve_target_name<'py>(
    py: Python<'py>,
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    selected_target: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyString>>> {
    let (name, error) =
        effective_config_target_name(py, project_config, local_config, selected_target)?;
    match error {
        Some(message) => Err(spec_error(py, message)?),
        None => Ok(name),
    }
}

/// Merge project target config with local developer overrides.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config, target_name))]
pub(crate) fn resolve_target_config<'py>(
    py: Python<'py>,
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
    target_name: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let (rows, error) = effective_config_target_fields(
        py,
        (project_config.clone(), local_config.clone()),
        target_name,
        &new_record(py, RecordId::TargetConfig, (), None)?,
    )?;
    let Some(rows) = rows else {
        let message: Bound<'py, PyString> = error.unwrap_or_else(|| PyString::new(py, ""));
        return Err(spec_error(py, message)?);
    };
    let values: Bound<'py, PyDict> = PyDict::new(py);
    let grouped: Vec<(&str, Bound<'py, PyDict>)> = TARGET_GROUPS
        .iter()
        .map(|(group, _)| (*group, PyDict::new(py)))
        .collect();
    for (group, name, value) in rows {
        match group {
            None => values.set_item(name, value)?,
            Some(group) => {
                if let Some((_, fields)) = grouped.iter().find(|(known, _)| *known == group) {
                    fields.set_item(name, value)?;
                }
            }
        }
    }
    for ((group, class_name), (_, fields)) in TARGET_GROUPS.iter().zip(&grouped) {
        let nested: Bound<'py, PyAny> = new_record(py, *class_name, (), Some(fields))?;
        values.set_item(*group, nested)?;
    }
    new_record(py, RecordId::TargetConfig, (), Some(&values))
}

/// Resolve the effective adapter name, allowing local override.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config))]
pub(crate) fn resolve_effective_adapter_name<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let local: Bound<'py, PyAny> = native_field(local_config, "adapter")?;
    if local.is_none() {
        return native_field(project_config, "adapter");
    }
    Ok(local)
}

/// Resolve declaration rendering over project configuration.
#[pyfunction]
#[pyo3(signature = (*, project_config, declaration_override))]
pub(crate) fn resolve_effective_collection_rendering<'py>(
    project_config: &Bound<'py, PyAny>,
    declaration_override: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if !declaration_override.is_none() {
        return Ok(declaration_override.clone());
    }
    native_field(project_config, "constants")?.getattr("collection_rendering")
}

/// `local` when set, else `project`.
fn override_or<'py>(local: Bound<'py, PyAny>, project: Bound<'py, PyAny>) -> Bound<'py, PyAny> {
    if local.is_none() { project } else { local }
}

/// Resolve scenario config, allowing local overrides to replace project rules.
#[pyfunction]
#[pyo3(signature = (*, project_config, local_config))]
pub(crate) fn resolve_effective_scenario_config<'py>(
    project_config: &Bound<'py, PyAny>,
    local_config: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_config.py();
    let project: Bound<'py, PyAny> = native_field(project_config, "scenario")?;
    let local: Bound<'py, PyAny> = native_field(local_config, "scenario")?;
    let overrides: Bound<'py, PyDict> = PyDict::new(py);
    for item in native_field(&project, "local_type_overrides")?
        .call_method0("items")?
        .try_iter()?
    {
        let (dialect, rules): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        overrides.set_item(dialect, py.get_type::<PyDict>().call1((rules,))?)?;
    }
    for item in native_field(&local, "local_type_overrides")?
        .call_method0("items")?
        .try_iter()?
    {
        let (dialect, rules): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        overrides
            .call_method1("setdefault", (dialect, PyDict::new(py)))?
            .call_method1("update", (rules,))?;
    }
    let project_limits: Bound<'py, PyAny> = native_field(&project, "snapshot_limits")?;
    let local_limits: Bound<'py, PyAny> = native_field(&local, "snapshot_limits")?;
    let limits: Vec<(&str, Bound<'py, PyAny>)> = SNAPSHOT_LIMITS
        .iter()
        .map(|field| {
            Ok((
                *field,
                override_or(
                    native_field(&local_limits, field)?,
                    native_field(&project_limits, field)?,
                ),
            ))
        })
        .collect::<PyResult<_>>()?;
    new_record_with(
        py,
        RecordId::ScenarioConfig,
        &[
            (
                "run_namespace",
                override_or(
                    native_field(&local, "run_namespace")?,
                    native_field(&project, "run_namespace")?,
                ),
            ),
            ("local_type_overrides", overrides.into_any()),
            (
                "snapshot_limits",
                new_record_with(py, RecordId::ScenarioSnapshotLimitsConfig, &limits)?,
            ),
        ],
    )
}

/// Return global and dialect-specific scenario local type override rules.
#[pyfunction]
#[pyo3(signature = (*, scenario_config, sql_analysis_dialect))]
fn scenario_local_type_overrides_for_dialect<'py>(
    scenario_config: &Bound<'py, PyAny>,
    sql_analysis_dialect: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = scenario_config.py();
    let rules: Bound<'py, PyAny> = native_field(scenario_config, "local_type_overrides")?;
    let overrides: Bound<'py, PyDict> = PyDict::new(py);
    overrides.call_method1(
        "update",
        (mapping_get_or(&rules, ALL_DIALECTS, PyDict::new(py))?,),
    )?;
    if !sql_analysis_dialect.is_none() {
        overrides.call_method1(
            "update",
            (mapping_get_or(
                &rules,
                sql_analysis_dialect,
                PyDict::new(py),
            )?,),
        )?;
    }
    Ok(overrides)
}

/// Whether `text` is empty or only whitespace as Python's `str.strip` sees it.
fn blank(text: &str) -> bool {
    text.chars()
        .all(|character| character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character))
}

/// Parse a one-, two-, or three-part loader destination.
#[pyfunction]
#[pyo3(signature = (*, destination, default_database, default_schema))]
pub(crate) fn loader_destination_parts<'py>(
    py: Python<'py>,
    destination: &str,
    default_database: Option<Bound<'py, PyAny>>,
    default_schema: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    loader_parts(py, destination, (default_database, default_schema))
}

pub(crate) fn loader_parts<'py>(
    py: Python<'py>,
    destination: &str,
    (default_database, default_schema): (Option<Bound<'py, PyAny>>, Option<Bound<'py, PyAny>>),
) -> PyResult<Bound<'py, PyAny>> {
    let parts: Vec<&str> = destination.split('.').collect();
    if !(1..=LOADER_QUALIFIED_TABLE_PARTS).contains(&parts.len())
        || parts.iter().any(|part| blank(part))
    {
        return Err(spec_error(
            py,
            PyString::new(
                py,
                &format!(
                    "Invalid loader destination '{destination}': expected 1 to 3 non-empty \
                 dot-separated parts (table, schema.table, or database.schema.table)"
                ),
            ),
        )?);
    }
    let none = || py.None().into_bound(py);
    let text = |part: &str| PyString::new(py, part).into_any();
    let (database, schema, table): (Bound<'py, PyAny>, Bound<'py, PyAny>, Bound<'py, PyAny>) =
        match parts.as_slice() {
            [table] => (
                default_database.unwrap_or_else(none),
                default_schema.unwrap_or_else(none),
                text(table),
            ),
            [schema, table] if parts.len() == LOADER_SCHEMA_TABLE_PARTS => (
                default_database.unwrap_or_else(none),
                text(schema),
                text(table),
            ),
            _ => (text(parts[0]), text(parts[1]), text(parts[2])),
        };
    new_record(
        py,
        RecordId::LoaderDestinationParts,
        (database, schema, table),
        None,
    )
}

/// Whether a family's optional `name_pattern` accepts `column_name`.
fn family_accepts(family: &Bound<'_, PyAny>, column_name: &str) -> PyResult<bool> {
    let pattern: Option<String> = native_field(family, "name_pattern")?.extract()?;
    let Some(pattern) = pattern else {
        return Ok(true);
    };
    NAME_PATTERNS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.contains_key(&pattern) {
            let compiled: NamePattern = compile_name_pattern(&pattern)
                .map_err(|error| PyValueError::new_err(format!("invalid name_pattern: {error}")))?;
            if cache.len() >= NAME_PATTERN_CACHE_LIMIT {
                cache.clear();
            }
            cache.insert(pattern.clone(), compiled);
        }
        Ok(cache
            .get(&pattern)
            .is_some_and(|compiled| compiled.matches(column_name)))
    })
}

/// Return declared dynamic families accepting a physical column name.
#[pyfunction]
#[pyo3(signature = (*, families, column_name))]
pub(crate) fn matching_dynamic_families<'py>(
    families: &Bound<'py, PyAny>,
    column_name: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    let mut matching: Vec<Bound<'py, PyAny>> = Vec::new();
    for family in families.try_iter()? {
        let family: Bound<'py, PyAny> = family?;
        if family_accepts(&family, column_name)? {
            matching.push(family);
        }
    }
    PyTuple::new(families.py(), matching)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(resolve_target_name, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_target_config, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_effective_adapter_name, module)?)?;
    module.add_function(wrap_pyfunction!(
        resolve_effective_collection_rendering,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(resolve_effective_scenario_config, module)?)?;
    module.add_function(wrap_pyfunction!(
        scenario_local_type_overrides_for_dialect,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(loader_destination_parts, module)?)?;
    module.add_function(wrap_pyfunction!(matching_dynamic_families, module)?)?;
    Ok(())
}
