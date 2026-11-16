//! Effective target, adapter, rendering, scenario and loader destination resolution.

use std::cell::RefCell;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyString};
use sqlbuild_model_config::name_patterns::main::compile_name_pattern::compile_name_pattern;
use sqlbuild_model_config::name_patterns::models::NamePattern;

use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

pub(crate) const TARGET_GROUPS: [(&str, RecordId); 4] = [
    ("clone", RecordId::ClonePolicy),
    ("execution_limits", RecordId::ExecutionLimitsConfig),
    ("diff", RecordId::TargetDiffConfig),
    ("warehouses", RecordId::TargetWarehousesConfig),
];
pub(crate) const SNAPSHOT_LIMITS: [&str; 4] = [
    "max_rows_per_relation",
    "max_total_rows",
    "max_bytes_per_relation",
    "max_total_bytes",
];
const ALL_DIALECTS: &str = "*";
pub(crate) const LOADER_SCHEMA_TABLE_PARTS: usize = 2;
pub(crate) const LOADER_QUALIFIED_TABLE_PARTS: usize = 3;
const NAME_PATTERN_CACHE_LIMIT: usize = 256;

thread_local! {
    static NAME_PATTERNS: RefCell<HashMap<String, NamePattern>> = RefCell::new(HashMap::new());
}

pub(crate) fn spec_error<'py>(py: Python<'py>, message: Bound<'py, PyString>) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::SpecConfig)?.call1((message,))?,
    ))
}

/// `local` when set, else `project`.
pub(crate) fn override_or<'py>(
    local: Bound<'py, PyAny>,
    project: Bound<'py, PyAny>,
) -> Bound<'py, PyAny> {
    if local.is_none() { project } else { local }
}

/// Return global and dialect-specific scenario local type override rules.
#[pyfunction]
#[pyo3(signature = (*, scenario_config, sql_analysis_dialect))]
pub(crate) fn scenario_local_type_overrides_for_dialect<'py>(
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
pub(crate) fn blank(text: &str) -> bool {
    text.chars()
        .all(|character| character.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&character))
}

/// Whether a family's optional `name_pattern` accepts `column_name`.
pub(crate) fn family_accepts(family: &Bound<'_, PyAny>, column_name: &str) -> PyResult<bool> {
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
