//! Per-model reasons a semantic check was only partial, for machine reports.

use std::collections::BTreeSet;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

use crate::bindings::_helpers::compile_context::delta::ModelCoverage;
use crate::bindings::_helpers::compiled_views::model_reads::{
    inferred_rows, query_sql, star_flags,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::semantic_shapes::delta_shapes::semantic_reasons;
use crate::bindings::_helpers::semantic_shapes::shapes::project_shapes;
use sqlbuild_analysis::semantic_validation::main::unknown_functions::unknown_functions;
use sqlbuild_project::types::InferredRow;

const TYPE_CHECKED_DIALECTS: [&str; 4] = ["duckdb", "snowflake", "postgresql", "bigquery"];
const SOURCE_KIND: &str = "source";
const OPEN_KINDS: [&str; 3] = [SOURCE_KIND, "ref", "dbt_ref"];
const UNKNOWN_FUNCTION: &str = "unknown function: ";

/// Partial-check reasons by model name; empty when SQL analysis is off.
#[pyfunction]
pub(crate) fn semantic_coverage<'py>(project: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = project.py();
    let reasons: Bound<'py, PyDict> = PyDict::new(py);
    if !native_field(&native_field(project, "settings")?, "sql_analysis")?.is_truthy()? {
        return Ok(reasons);
    }
    let shapes: Bound<'py, PyDict> = project_shapes(project, None)?;
    let dialect: Bound<'py, PyAny> = native_field(project, "sql_analysis_dialect")?;
    let dialect: String = if dialect.is_truthy()? {
        dialect.extract()?
    } else {
        "generic".to_owned()
    };
    let mut declared: Vec<String> = Vec::new();
    for function in native_field(project, "functions")?.try_iter()? {
        declared.push(native_field(&function?, "name")?.extract()?);
    }
    let mut order: Vec<(String, Vec<String>)> = Vec::new();
    let mut facts: Vec<ModelCoverage> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        let mut rows: Vec<(String, String)> = Vec::new();
        for row in native_field(&model, "reference_rows")?.try_iter()? {
            let row: Bound<'py, PyAny> = row?;
            rows.push((
                row.get_item(0)?.str()?.extract()?,
                row.get_item(1)?.extract()?,
            ));
        }
        let mut blocked: bool = false;
        for item in native_field(&model, "binding_diagnostics")?.try_iter()? {
            if native_field(&item?, "is_error")?.is_truthy()? {
                blocked = true;
                break;
            }
        }
        let opted: Bound<'py, PyAny> = native_field(&native_field(&model, "config")?, "values")?
            .call_method1("get", ("sql_analysis",))?;
        let disabled: bool = opted.is(PyBool::new(py, false));
        let partial: BTreeSet<String> = if disabled {
            BTreeSet::new()
        } else {
            own_reasons((&model, &name), &rows, (&shapes, &dialect, &declared))?
        };
        facts.push(ModelCoverage {
            name: name.clone(),
            disabled,
            blocked,
            partial: partial.into_iter().collect(),
        });
        order.push((
            name,
            rows.into_iter().map(|(_, upstream)| upstream).collect(),
        ));
    }
    for (name, partial) in semantic_reasons(order, facts)? {
        reasons.set_item(name, PyTuple::new(py, partial)?)?;
    }
    Ok(reasons)
}

/// A model's partial-check reasons apart from those its invalid dependencies give it.
fn own_reasons<'py>(
    (model, name): (&Bound<'py, PyAny>, &str),
    rows: &[(String, String)],
    (shapes, dialect, declared): (&Bound<'py, PyDict>, &str, &[String]),
) -> PyResult<BTreeSet<String>> {
    let mut partial: BTreeSet<String> = BTreeSet::new();
    for (kind, upstream) in rows {
        if OPEN_KINDS.contains(&kind.as_str()) && !shapes.contains(upstream)? {
            partial.insert(if kind == SOURCE_KIND {
                format!("open source: {upstream}")
            } else {
                format!("open input: {upstream}")
            });
        }
    }
    let inferred: Vec<InferredRow> = inferred_rows(model)?.unwrap_or_default();
    if inferred.is_empty() {
        partial.insert("output shape could not be inferred".to_owned());
    } else if star_flags(model)?.0 && !shapes.contains(name)? {
        partial.insert("unresolved star over open inputs".to_owned());
    }
    if inferred
        .iter()
        .any(|(_, column_type, _)| column_type.is_none())
    {
        partial.insert("some output types are unknown".to_owned());
    }
    let sql: Bound<'py, PyAny> = query_sql(model)?;
    if sql.is_truthy()? {
        let sql: String = sql.extract()?;
        if let Ok(names) = unknown_functions(&sql, dialect, declared) {
            partial.extend(
                names
                    .into_iter()
                    .map(|name| format!("{UNKNOWN_FUNCTION}{name}")),
            );
        }
    }
    if !TYPE_CHECKED_DIALECTS.contains(&dialect) {
        partial.insert("type checking awaits dialect coercion support".to_owned());
    }
    Ok(partial)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(semantic_coverage, module)?)
}
