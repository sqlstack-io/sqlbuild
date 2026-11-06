//! The model and hook SQL assembly validates, and the SQL standalone pivot proofs read.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_model_config::header_metadata::models::DynamicFamily;
use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use crate::bindings::_helpers::attachments::cursor_intrinsics::analysis_cursor_sql;
use crate::bindings::_helpers::compiled_views::model_input_view::attached_schema;
use crate::bindings::_helpers::project_assembly::syntax_validation::hook_sql_statements;
use crate::bindings::_helpers::records::fields::field as native_field;

const HOOK_NAMES: [&str; 2] = ["pre_hooks", "post_hooks"];

/// Native SQL and dynamic family specs indexed by their model's position.
type PivotModelRows = Vec<(usize, (String, Vec<DynamicFamily>))>;

fn config_value<'py>(model_input: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    native_field(&native_field(model_input, "config")?, "values")?.call_method1("get", (key,))
}

fn model_name(model_input: &Bound<'_, PyAny>) -> PyResult<String> {
    native_field(
        &native_field(&native_field(model_input, "model_file")?, "file_path")?,
        "stem",
    )?
    .extract()
}

/// The model's `@@@name` placeholder defaults as strings, or `None`.
#[pyfunction]
pub(crate) fn model_placeholders<'py>(
    model_input: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let raw: Bound<'py, PyAny> = config_value(model_input, "placeholders")?;
    let Ok(raw) = raw.downcast::<PyDict>() else {
        return Ok(None);
    };
    let placeholders: Bound<'py, PyDict> = PyDict::new(model_input.py());
    for (key, value) in raw.iter() {
        placeholders.set_item(key.str()?, value.str()?)?;
    }
    Ok(Some(placeholders))
}

/// The model query with cursor intrinsics replaced by typed literals for static analysis.
fn analysis_sql<'py>(model_input: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
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

fn syntax_checks<'py>(
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

/// Each model's `(sql, placeholders)` that assembly passes to syntax validation.
#[pyfunction]
#[pyo3(signature = (*, model_inputs, analysis_model_names, analysis_succeeded))]
pub(crate) fn model_syntax_checks<'py>(
    model_inputs: &Bound<'py, PyAny>,
    analysis_model_names: &Bound<'py, PyAny>,
    analysis_succeeded: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let checks: Vec<Bound<'py, PyTuple>> = model_inputs
        .try_iter()?
        .map(|model_input| syntax_checks(&model_input?, analysis_model_names, analysis_succeeded))
        .collect::<PyResult<_>>()?;
    PyTuple::new(model_inputs.py(), checks)
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

/// Native-only pivot requests; the public wrapper alone projects contract records.
pub(crate) fn standalone_pivot_rows(
    model_inputs: &Bound<'_, PyAny>,
    analysed: &Bound<'_, PyAny>,
) -> PyResult<PivotModelRows> {
    let mut models: PivotModelRows = Vec::new();
    for (index, model_input) in model_inputs.try_iter()?.enumerate() {
        let model_input = model_input?;
        let Some(schema) = attached_schema(&model_input)? else {
            continue;
        };
        let families = schema.dynamic_columns;
        if families.is_empty() || analysed.contains(model_name(&model_input)?)? {
            continue;
        }
        let query_sql: String = analysis_sql(&model_input)?.extract()?;
        let sql: String = match model_placeholders(&model_input)? {
            Some(placeholders) if !placeholders.is_empty() => {
                let pairs: Vec<(String, String)> = placeholders
                    .iter()
                    .map(|(key, value)| Ok((key.extract()?, value.extract()?)))
                    .collect::<PyResult<_>>()?;
                placeholder_defaults(&query_sql, &pairs)
            }
            _ => query_sql,
        };
        models.push((index, (sql, families)));
    }
    Ok(models)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(model_placeholders, module)?)?;
    module.add_function(wrap_pyfunction!(model_syntax_checks, module)?)?;
    module.add_function(wrap_pyfunction!(standalone_pivot_models, module)?)?;
    Ok(())
}
