//! The model and hook SQL assembly validates, and the SQL standalone pivot proofs read.

use crate::project_assembly::main::model_placeholders::model_placeholders;
use crate::project_assembly::main::standalone_pivot_rows::standalone_pivot_rows;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_model_config::header_metadata::models::DynamicFamily;

use crate::project_assembly::_helpers::syntax_validation::hook_sql_statements;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::attachments::main::analysis_cursor_sql::analysis_cursor_sql;

const HOOK_NAMES: [&str; 2] = ["pre_hooks", "post_hooks"];

/// Native SQL and dynamic family specs indexed by their model's position.
pub(crate) type PivotModelRows = Vec<(usize, (String, Vec<DynamicFamily>))>;

pub(crate) fn config_value<'py>(
    model_input: &Bound<'py, PyAny>,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    native_field(&native_field(model_input, "config")?, "values")?.call_method1("get", (key,))
}

pub(crate) fn model_name(model_input: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(
        &native_field(&native_field(model_input, "model_file")?, "file_path")?,
        "stem",
    )?
    .extract()
}

/// The model query with cursor intrinsics replaced by typed literals for static analysis.
pub(crate) fn analysis_sql<'py>(model_input: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model_input.py();
    Ok(PyString::new(
        py,
        &analysis_cursor_sql(
            py,
            &native_field(model_input, "query_sql")?.extract::<String>()?,
            &config_value(model_input, "cursor_type")?,
        )?,
    )
    .into_any())
}

pub(crate) fn syntax_checks<'py>(
    model_input: &Bound<'py, PyAny>,
    analysis_model_names: &Bound<'py, PyAny>,
    analysis_succeeded: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = model_input.py();
    let name: String = model_name(model_input)?;
    if !native_field(model_input, "sql_validation_enabled")?.is_truthy()?
        || (!analysis_model_names.is_none() && !analysis_model_names.contains(&name)?)
    {
        return Ok(PyTuple::empty(py));
    }
    let placeholders: Bound<'py, PyAny> = match model_placeholders(model_input)? {
        Some(placeholders) => placeholders.into_any(),
        None => py.None().into_bound(py),
    };
    let mut statements: Vec<Bound<'py, PyAny>> = Vec::new();
    for hook_name in HOOK_NAMES {
        for statement in hook_sql_statements(config_value(model_input, hook_name)?)? {
            statements.push(PyString::new(py, &statement).into_any());
        }
    }
    if !analysis_succeeded.contains(&name)? {
        statements.push(analysis_sql(model_input)?);
    }
    let checks: Vec<Bound<'py, PyTuple>> = statements
        .into_iter()
        .map(|statement| PyTuple::new(py, [statement, placeholders.clone()]))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, checks)
}

/// `(sql, families)` by model index for each pivot model no analysis covered.
#[pyfunction]
#[pyo3(signature = (*, model_inputs, analysed))]
pub(crate) fn standalone_pivot_models<'py>(
    model_inputs: &Bound<'py, PyAny>,
    analysed: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = model_inputs.py();
    let models: Bound<'py, PyDict> = PyDict::new(py);
    for (index, (sql, _)) in standalone_pivot_rows(model_inputs, analysed)? {
        let model = model_inputs.get_item(index)?;
        let entry = native_field(&model, "schema_entry")?;
        models.set_item(index, (sql, native_field(&entry, "dynamic_columns")?))?;
    }
    Ok(models)
}
