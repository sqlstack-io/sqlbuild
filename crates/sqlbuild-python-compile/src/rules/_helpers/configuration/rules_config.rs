//! Rules configuration loaded natively and returned as the Python configuration models.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use serde_json::Value;
use sqlbuild_rules::models::RulesConfig;

use sqlbuild_python_base::boundary::main::json_value_to_python::json_value_to_python;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

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

pub(crate) fn config_object<'py>(
    py: Python<'py>,
    config: &RulesConfig,
) -> PyResult<Bound<'py, PyAny>> {
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
            (
                "select",
                match &config.select {
                    Some(select) => strings(py, select)?,
                    None => py.None().into_bound(py),
                },
            ),
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

pub(crate) fn rules_error(
    py: Python<'_>,
    class: ErrorId,
    message: String,
    help: Option<String>,
) -> PyErr {
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
