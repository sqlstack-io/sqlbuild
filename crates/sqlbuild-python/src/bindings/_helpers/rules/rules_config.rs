//! Rules configuration loaded natively and returned as the Python configuration models.

use std::path::{Path, PathBuf};

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{IntoPyObjectExt, Py, pyfunction, wrap_pyfunction};
use serde_json::Value;
use sqlbuild_rules::configuration::errors::RulesConfigLoadError;
use sqlbuild_rules::configuration::main::load_rules_config::load_rules_config as load_config;
use sqlbuild_rules::models::RulesConfig;

use crate::bindings::_helpers::boundary::config_values::json_value_to_python;
use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

struct Models<'py> {
    py: Python<'py>,
}

impl<'py> Models<'py> {
    fn build(
        &self,
        class: RecordId,
        fields: &[(&str, Bound<'py, PyAny>)],
    ) -> PyResult<Bound<'py, PyAny>> {
        let arguments: Bound<'py, PyDict> = PyDict::new(self.py);
        for (name, value) in fields {
            arguments.set_item(name, value)?;
        }
        new_record(self.py, class, (), Some(&arguments))
    }
}

fn strings<'py>(py: Python<'py>, values: &[String]) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyTuple::new(py, values)?.into_any())
}

fn text<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyAny>> {
    value.into_bound_py_any(py)
}

fn integers<'py>(
    py: Python<'py>,
    values: &std::collections::BTreeMap<String, u32>,
) -> PyResult<Bound<'py, PyAny>> {
    values.into_bound_py_any(py)
}

/// A scalar option as Python reads it from JSON; floats, tables and nulls are dropped.
fn option_scalar<'py>(py: Python<'py>, value: &Value) -> PyResult<Option<Bound<'py, PyAny>>> {
    match value {
        Value::Bool(flag) => Ok(Some(flag.into_bound_py_any(py)?)),
        Value::String(item) => Ok(Some(item.into_bound_py_any(py)?)),
        Value::Number(number) => match (number.as_i64(), number.as_u64()) {
            (Some(integer), _) => Ok(Some(integer.into_bound_py_any(py)?)),
            (None, Some(integer)) => Ok(Some(integer.into_bound_py_any(py)?)),
            (None, None) => Ok(None),
        },
        _ => Ok(None),
    }
}

fn rule_options<'py>(py: Python<'py>, config: &RulesConfig) -> PyResult<Bound<'py, PyAny>> {
    let result: Bound<'py, PyDict> = PyDict::new(py);
    for (code, options) in &config.rule_options {
        let values: Bound<'py, PyDict> = PyDict::new(py);
        for (name, value) in options {
            if let Value::Array(items) = value {
                let tuple: Vec<Bound<'py, PyAny>> = items
                    .iter()
                    .map(|item| json_value_to_python(py, item))
                    .collect::<PyResult<_>>()?;
                values.set_item(name, PyTuple::new(py, tuple)?)?;
            } else if let Some(scalar) = option_scalar(py, value)? {
                values.set_item(name, scalar)?;
            }
        }
        result.set_item(code, values)?;
    }
    Ok(result.into_any())
}

fn tables<'py, T>(
    py: Python<'py>,
    items: &[T],
    build: impl Fn(&T) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let values: Vec<Bound<'py, PyAny>> = items.iter().map(build).collect::<PyResult<_>>()?;
    Ok(PyTuple::new(py, values)?.into_any())
}

fn config_object<'py>(py: Python<'py>, config: &RulesConfig) -> PyResult<Bound<'py, PyAny>> {
    let models = Models { py };
    let threshold_overrides = tables(py, &config.threshold_overrides, |item| {
        models.build(
            RecordId::ThresholdOverride,
            &[
                ("paths", strings(py, &item.paths)?),
                ("thresholds", integers(py, &item.thresholds)?),
                ("reason", text(py, &item.reason)?),
            ],
        )
    })?;
    let rule_exceptions = tables(py, &config.rule_exceptions, |item| {
        models.build(
            RecordId::RuleExemption,
            &[
                ("rule", text(py, &item.rule)?),
                ("path", text(py, &item.path)?),
                ("reason", text(py, &item.reason)?),
                ("relation", item.relation.clone().into_bound_py_any(py)?),
                ("cte", item.cte.clone().into_bound_py_any(py)?),
            ],
        )
    })?;
    let graph_edge_exceptions = tables(py, &config.graph_edge_exceptions, |item| {
        models.build(
            RecordId::GraphEdgeExclusion,
            &[
                ("consumer", text(py, &item.consumer)?),
                ("dependency", text(py, &item.dependency)?),
                ("reason", text(py, &item.reason)?),
            ],
        )
    })?;
    let rule_ignores = tables(py, &config.rule_ignores, |item| {
        models.build(
            RecordId::RuleIgnore,
            &[
                ("rules", strings(py, &item.rules)?),
                ("paths", strings(py, &item.paths)?),
                ("reason", text(py, &item.reason)?),
                ("selectors", strings(py, &item.selectors)?),
            ],
        )
    })?;
    let select_star_allow = tables(py, &config.select_star_allow, |item| {
        models.build(
            RecordId::SelectStarAllow,
            &[
                ("paths", strings(py, &item.paths)?),
                ("reason", text(py, &item.reason)?),
            ],
        )
    })?;
    let mut layout: Vec<(&str, Bound<'py, PyAny>)> =
        vec![("domain_roots", strings(py, &config.layout.domain_roots)?)];
    if !config.layout.levels.is_empty() {
        layout.push(("levels", strings(py, &config.layout.levels)?));
    }
    models.build(
        RecordId::RulesConfig,
        &[
            ("select", strings(py, &config.select)?),
            ("ignore", strings(py, &config.ignore)?),
            ("thresholds", integers(py, &config.thresholds)?),
            ("threshold_overrides", threshold_overrides),
            ("rule_options", rule_options(py, config)?),
            ("rule_exceptions", rule_exceptions),
            ("graph_edge_exceptions", graph_edge_exceptions),
            ("rule_ignores", rule_ignores),
            ("select_star_allow", select_star_allow),
            ("domains", strings(py, &config.domains)?),
            (
                "approved_source_tokens",
                strings(py, &config.approved_source_tokens)?,
            ),
            (
                "retired_source_tokens",
                config.retired_source_tokens.clone().into_bound_py_any(py)?,
            ),
            (
                "sql_tests",
                models.build(
                    RecordId::SqlTestRulesConfig,
                    &[(
                        "pipeline_directory",
                        text(py, &config.sql_tests.pipeline_directory)?,
                    )],
                )?,
            ),
            ("layout", models.build(RecordId::LayoutConfig, &layout)?),
            (
                "cache",
                models.build(
                    RecordId::RulesCacheConfig,
                    &[("enabled", config.cache.enabled.into_bound_py_any(py)?)],
                )?,
            ),
        ],
    )
}

fn rules_error(py: Python<'_>, class: ErrorId, message: String, help: Option<String>) -> PyErr {
    let raised = || -> PyResult<PyErr> {
        let error_class: Bound<'_, PyAny> = error_class(py, class)?;
        let error: Bound<'_, PyAny> = match help {
            Some(help) => {
                let arguments: Bound<'_, PyDict> = PyDict::new(py);
                arguments.set_item("help", help)?;
                error_class.call((message,), Some(&arguments))?
            }
            None => error_class.call1((message,))?,
        };
        Ok(PyErr::from_value(error))
    };
    raised().unwrap_or_else(|error| error)
}

/// The project's validated rules configuration as `RulesConfig`.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn load_rules_config(py: Python<'_>, project_dir: PathBuf) -> PyResult<Py<PyAny>> {
    load_rules_config_with_cache(py, project_dir, true)
}

/// The project's rules configuration, with the Rules cache off when `cache` is `false`.
pub(crate) fn load_rules_config_with_cache(
    py: Python<'_>,
    project_dir: PathBuf,
    cache: bool,
) -> PyResult<Py<PyAny>> {
    let mut config: RulesConfig = match rules_config_of(&project_dir)? {
        Ok(config) => config,
        Err(invalid) => return Err(invalid.into_pyerr(py)),
    };
    config.cache.enabled &= cache;
    Ok(config_object(py, &config)?.unbind())
}

/// Why a project's rules configuration is invalid.
pub(crate) enum InvalidRulesConfig {
    /// The configuration could not be read: a `RulesError`.
    Unreadable(String),
    /// It uses a setting SQLBuild rejects: a `RulesConfigError` with help.
    Forbidden { message: String, help: String },
}

impl InvalidRulesConfig {
    fn into_pyerr(self, py: Python<'_>) -> PyErr {
        match self {
            Self::Unreadable(message) => rules_error(py, ErrorId::Rules, message, None),
            Self::Forbidden { message, help } => {
                rules_error(py, ErrorId::RulesConfig, message, Some(help))
            }
        }
    }
}

/// The project's validated rules configuration, or why it is invalid.
pub(crate) fn rules_config_of(
    project_dir: &Path,
) -> PyResult<Result<RulesConfig, InvalidRulesConfig>> {
    let project_dir: PathBuf = project_dir
        .canonicalize()
        .or_else(|_| std::path::absolute(project_dir))?;
    match compiler_guard(|| Ok(load_config(&project_dir)))? {
        Ok(config) => Ok(Ok(config)),
        Err(RulesConfigLoadError::Invalid(message)) => {
            Ok(Err(InvalidRulesConfig::Unreadable(message)))
        }
        Err(RulesConfigLoadError::Policy(error)) => Ok(Err(InvalidRulesConfig::Forbidden {
            message: error.message,
            help: error.help,
        })),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(load_rules_config, module)?)?;
    Ok(())
}
