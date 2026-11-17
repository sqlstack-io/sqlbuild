//! Render executable SQL test and scenario comparison queries from planned entries.

use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use pyo3::{FromPyObject, pyfunction};
use serde_json::{Value, json};
use sqlbuild_analysis::compiler::main::missing_expected_columns::missing_expected_columns;
use sqlbuild_analysis::compiler::main::scenario_expected_comparison_sql::scenario_expected_comparison_sql;
use sqlbuild_analysis::compiler::main::sql_test_difference_sampling::render_difference_sample_json;
use sqlbuild_analysis::compiler::main::sql_test_rendering::render_json;

use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::types::CompilerDetach;

const RENDER_WORKERS: usize = 4;
const DEFAULT_SET_DIFFERENCE: &str = "EXCEPT";
const TOP_CLAUSE_DIALECT: &str = "tsql";

/// Operator, dialect and optional probed chain step of one comparison render.
pub(crate) struct RenderSettings {
    pub(crate) set_difference_operator: String,
    pub(crate) sql_analysis_dialect: Option<String>,
    pub(crate) probe_step_index: Option<usize>,
    pub(crate) output_step_index: Option<usize>,
    pub(crate) actual_relations: HashMap<usize, String>,
}

impl RenderSettings {
    fn read(options: Option<&Bound<'_, PyDict>>) -> PyResult<Self> {
        Ok(Self {
            set_difference_operator: optional_value(options, "set_difference_operator")?
                .unwrap_or_else(|| DEFAULT_SET_DIFFERENCE.to_owned()),
            sql_analysis_dialect: optional_value(options, "sql_analysis_dialect")?,
            probe_step_index: optional_value(options, "probe_step_index")?,
            output_step_index: optional_value(options, "output_step_index")?,
            actual_relations: optional_value(options, "actual_relations")?.unwrap_or_default(),
        })
    }
}

fn optional_value<T: for<'py> FromPyObject<'py>>(
    options: Option<&Bound<'_, PyDict>>,
    name: &str,
) -> PyResult<Option<T>> {
    let value: Option<Bound<'_, PyAny>> = match options {
        Some(options) => options.get_item(name)?,
        None => None,
    };
    match value {
        Some(value) if !value.is_none() => Ok(Some(value.extract()?)),
        _ => Ok(None),
    }
}

/// Comparison SQL of each planned SQL test, or of one chain step's actual-output probe.
#[pyfunction]
#[pyo3(signature = (test_entries, **options))]
pub(crate) fn render_sql_test_comparison_sql(
    py: Python<'_>,
    test_entries: Vec<Bound<'_, PyAny>>,
    options: Option<&Bound<'_, PyDict>>,
) -> PyResult<Vec<String>> {
    rendered_comparisons(py, &test_entries, &RenderSettings::read(options)?)
}

pub(crate) fn rendered_comparisons(
    py: Python<'_>,
    test_entries: &[Bound<'_, PyAny>],
    settings: &RenderSettings,
) -> PyResult<Vec<String>> {
    if test_entries.is_empty() {
        return Ok(Vec::new());
    }
    let mut requests: Vec<Value> = Vec::with_capacity(test_entries.len());
    for entry in test_entries {
        let mut request: Value = comparison_request(
            entry,
            &settings.set_difference_operator,
            settings.sql_analysis_dialect.as_deref(),
        )?;
        if let Some(fields) = request.as_object_mut() {
            if let Some(index) = settings.probe_step_index {
                fields.insert("probeStepIndex".to_owned(), json!(index));
            }
            if let Some(index) = settings.output_step_index {
                fields.insert("outputStepIndex".to_owned(), json!(index));
            }
            if !settings.actual_relations.is_empty() {
                fields.insert(
                    "actualRelations".to_owned(),
                    json!(settings.actual_relations),
                );
            }
        }
        requests.push(request);
    }
    let request_json: String =
        serde_json::to_string(&json!({"requests": requests, "workers": RENDER_WORKERS}))
            .map_err(value_error)?;
    let response: String = py
        .compiler_detach(|| render_json(&request_json))
        .map_err(value_error)?;
    let rendered: Vec<Value> = serde_json::from_str(&response).map_err(value_error)?;
    rendered
        .iter()
        .map(|item| {
            item.get("sql")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| value_error("native SQL-test rendering returned an invalid result"))
        })
        .collect()
}

/// The native comparison render request of one planned SQL test.
pub(crate) fn comparison_request(
    entry: &Bound<'_, PyAny>,
    set_difference_operator: &str,
    sql_analysis_dialect: Option<&str>,
) -> PyResult<Value> {
    let chain: Vec<Value> = native_field(entry, "chain")?
        .try_iter()?
        .map(|step| chain_step_request(&step?))
        .collect::<PyResult<_>>()?;
    let assertions: Vec<Value> = native_field(entry, "assertions")?
        .try_iter()?
        .map(|assertion| {
            let assertion: Bound<'_, PyAny> = assertion?;
            Ok(json!({
                "name": native_field(&assertion, "name")?.extract::<String>()?,
                "resolvedSql": native_field(&assertion, "resolved_sql")?.extract::<String>()?,
                "liftedCtes": native_field(&assertion, "lifted_ctes")?.extract::<Vec<(String, String)>>()?,
                "comparisonBodySql": native_field(&assertion, "comparison_body_sql")?.extract::<Option<String>>()?,
            }))
        })
        .collect::<PyResult<_>>()?;
    Ok(json!({
        "chain": chain,
        "assertions": assertions,
        "sqlAnalysisEnabled": native_field(entry, "sql_analysis_enabled")?.extract::<bool>()?,
        "setDifferenceOperator": set_difference_operator,
        "sqlAnalysisDialect": sql_analysis_dialect,
    }))
}

/// The native request of one planned chain step.
pub(crate) fn chain_step_request(step: &Bound<'_, PyAny>) -> PyResult<Value> {
    Ok(json!({
        "modelName": native_field(step, "model_name")?.extract::<String>()?,
        "resolvedSql": native_field(step, "resolved_sql")?.extract::<String>()?,
        "expectedCteSql": native_field(step, "expected_cte_sql")?.extract::<Option<String>>()?,
        "liftedCtes": native_field(step, "lifted_ctes")?.extract::<Vec<(String, String)>>()?,
        "comparisonBodySql": native_field(step, "comparison_body_sql")?.extract::<Option<String>>()?,
        "expectedColumns": native_field(step, "expected_columns")?.extract::<Option<Vec<String>>>()?,
        "expectedLiftedCtes": native_field(step, "expected_lifted_ctes")?.extract::<Vec<(String, String)>>()?,
    }))
}

/// A bounded query for one direction of an expected-output difference of one chain step.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn render_sql_test_difference_sample_sql(
    py: Python<'_>,
    options: Option<&Bound<'_, PyDict>>,
) -> PyResult<String> {
    let request = Request::new("render_sql_test_difference_sample_sql", options)?;
    let test_entry: Bound<'_, PyAny> = request.get("test_entry")?;
    let step: Bound<'_, PyAny> = request.get("step")?;
    if native_field(&step, "expected_cte_sql")?.is_none() {
        return Ok(String::new());
    }
    let settings: RenderSettings = RenderSettings::read(options)?;
    let request_json: String = serde_json::to_string(&json!({
        "step": chain_step_request(&step)?,
        "sqlAnalysisEnabled": native_field(&test_entry, "sql_analysis_enabled")?.extract::<bool>()?,
        "setDifferenceOperator": settings.set_difference_operator,
        "sqlAnalysisDialect": settings.sql_analysis_dialect,
        "direction": request.get("direction")?.extract::<String>()?,
        "sampleLimit": request.get("sample_limit")?.extract::<i64>()?,
        "useTopClause": settings.sql_analysis_dialect.as_deref() == Some(TOP_CLAUSE_DIALECT),
        "actualRelation": optional_value::<String>(options, "actual_relation")?,
    }))
    .map_err(value_error)?;
    let response: String = py
        .compiler_detach(|| render_difference_sample_json(&request_json))
        .map_err(value_error)?;
    let rendered: Value = serde_json::from_str(&response).map_err(value_error)?;
    rendered
        .get("sql")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| value_error("native SQL-test difference rendering returned invalid SQL"))
}

/// Describe listed expected columns absent from casefolded model output names.
#[pyfunction]
#[pyo3(signature = (*, model_name, expected_columns, available_columns))]
pub(crate) fn describe_missing_expected_columns(
    model_name: &str,
    expected_columns: Vec<String>,
    available_columns: Bound<'_, PyAny>,
) -> PyResult<Option<String>> {
    let available: Vec<String> = available_columns
        .try_iter()?
        .map(|column| column?.extract::<String>())
        .collect::<PyResult<_>>()?;
    Ok(missing_expected_columns(
        model_name,
        &expected_columns,
        &available,
    ))
}

/// SQL comparing a scenario's actual output with its expected SQL on the compared columns.
#[pyfunction]
#[pyo3(signature = (*, actual_sql, expected_sql, set_difference_operator, compared_columns = None))]
pub(crate) fn build_scenario_expected_comparison_sql(
    actual_sql: &str,
    expected_sql: &str,
    set_difference_operator: &str,
    compared_columns: Option<Vec<String>>,
) -> String {
    scenario_expected_comparison_sql(
        actual_sql,
        expected_sql,
        compared_columns.as_deref(),
        set_difference_operator,
    )
}
