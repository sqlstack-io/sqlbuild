//! Cursor intrinsic validation, rejection and rendering.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult};
use pyo3::types::{PyString, PyStringMethods};
use pyo3::{Python, pyfunction, wrap_pyfunction};
use sqlbuild_analysis::sql_macros::models::TextSplice;
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics::replace_intrinsics;
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics_with_spans::replace_intrinsics_with_spans;
use sqlbuild_attachments::cursor_intrinsics::main::validated_model_intrinsics::validated_model_intrinsics;
use sqlbuild_attachments::cursor_intrinsics::models::IntrinsicModel;

use crate::bindings::_helpers::boundary::panics::compiler_guard;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::ids::ErrorId;

const INCREMENTAL_MATERIALIZATION: &str = "incremental";
const CURSOR_START: &str = "__cursor_start";
const CURSOR_START_CALL: &str = "__cursor_start()";
const CURSOR_END_CALL: &str = "__cursor_end()";
const INTEGER_CURSOR_TYPE: &str = "integer";
const INTEGER_ANALYSIS_BOUND: &str = "CAST(0 AS BIGINT)";
const TIMESTAMP_ANALYSIS_BOUND: &str = "CAST('2000-01-01 00:00:00' AS TIMESTAMP)";
const MODEL_SQL_CONTEXT: &str = "Model SQL";
const SQL_CONTEXT: &str = "SQL";
/// Executor placeholder replaced with a microbatch's start bound.
pub(crate) const MICROBATCH_START_SENTINEL: &str = "__SQB_CURSOR_START__";
/// Executor placeholder replaced with a microbatch's end bound.
pub(crate) const MICROBATCH_END_SENTINEL: &str = "__SQB_CURSOR_END__";
const RESERVED_CURSOR_MARKERS: [&str; 2] = [MICROBATCH_START_SENTINEL, MICROBATCH_END_SENTINEL];

/// The reserved executor bound markers no authored SQL may contain.
pub(crate) fn reserved_cursor_markers() -> Vec<String> {
    RESERVED_CURSOR_MARKERS
        .iter()
        .map(|marker| (*marker).to_owned())
        .collect()
}

/// A `CompileInputError` carrying `message`.
pub(crate) fn compile_input_error(py: Python<'_>, message: String) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::CompileInput)?.call1((message,))?,
    ))
}

/// `text` when it is a string, else `None`.
fn text_value(value: &Bound<'_, PyAny>) -> Option<String> {
    match value.downcast::<PyString>() {
        Ok(text) => Some(text.to_string_lossy().into_owned()),
        Err(_) => None,
    }
}

/// `sql` with each intrinsic replaced by its bound and whether any was, raising for `context`.
fn replaced(
    py: Python<'_>,
    sql: &str,
    context: &str,
    (start, end): (&str, &str),
) -> PyResult<(String, bool)> {
    let outcome: Result<(String, bool), String> = compiler_guard(|| {
        Ok(replace_intrinsics(sql, context, |name| {
            if name == CURSOR_START {
                start.to_owned()
            } else {
                end.to_owned()
            }
        }))
    })?;
    outcome.or_else(|message| Err(compile_input_error(py, message)?))
}

/// One model query with canonical intrinsic calls, raising when the model may not use them.
pub(crate) fn validated_model_cursor_sql(
    py: Python<'_>,
    sql: &str,
    (model_name, config_values): (&str, &Bound<'_, PyAny>),
) -> PyResult<String> {
    let incremental: bool = text_value(&mapping_get(config_values, "materialized")?)
        .is_some_and(|materialized| materialized == INCREMENTAL_MATERIALIZATION);
    let cursor: Option<String> = text_value(&mapping_get(config_values, "cursor")?);
    let reserved_markers: Vec<String> = reserved_cursor_markers();
    let outcome: Result<String, String> = compiler_guard(|| {
        Ok(validated_model_intrinsics(
            sql,
            &reserved_markers,
            &IntrinsicModel {
                name: model_name,
                incremental,
                cursor: cursor.as_deref(),
            },
        ))
    })?;
    outcome.or_else(|message| Err(compile_input_error(py, message)?))
}

/// `sql` with intrinsics replaced by stable typed literals, only for static SQL analysis.
pub(crate) fn analysis_cursor_sql(
    py: Python<'_>,
    sql: &str,
    cursor_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let literal: &str = analysis_bound(text_value(cursor_type).as_deref());
    Ok(replaced(py, sql, MODEL_SQL_CONTEXT, (literal, literal))?.0)
}

/// `analysis_cursor_sql` with each replaced call as a splice of `sql`.
pub(crate) fn analysis_cursor_splices(
    sql: &str,
    cursor_type: Option<&str>,
) -> Result<(String, Vec<TextSplice>), String> {
    let literal: &str = analysis_bound(cursor_type);
    let (replaced, spans) =
        replace_intrinsics_with_spans(sql, MODEL_SQL_CONTEXT, |_| literal.to_owned())?;
    let splices: Vec<TextSplice> = spans
        .iter()
        .map(|span| TextSplice {
            source: (span.source_start, span.source_end),
            output: (span.output_start, span.output_end),
        })
        .collect();
    Ok((replaced, splices))
}

/// The literal static analysis reads for every cursor intrinsic of a model with `cursor_type`.
fn analysis_bound(cursor_type: Option<&str>) -> &'static str {
    if cursor_type == Some(INTEGER_CURSOR_TYPE) {
        INTEGER_ANALYSIS_BOUND
    } else {
        TIMESTAMP_ANALYSIS_BOUND
    }
}

/// One model query with canonical intrinsic calls, raising `CompileInputError` when invalid.
#[pyfunction]
#[pyo3(signature = (*, sql, config_values, model_name))]
fn validated_model_cursor_intrinsics(
    py: Python<'_>,
    sql: &str,
    config_values: &Bound<'_, PyAny>,
    model_name: &str,
) -> PyResult<String> {
    validated_model_cursor_sql(py, sql, (model_name, config_values))
}

/// `sql` with intrinsics replaced by typed literals of `cursor_type` for static analysis.
#[pyfunction]
#[pyo3(signature = (*, sql, cursor_type))]
fn cursor_intrinsics_analysis_sql(
    py: Python<'_>,
    sql: &str,
    cursor_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    analysis_cursor_sql(py, sql, cursor_type)
}

/// Whether `sql` uses cursor intrinsics, and `sql` with both bounds rendered when given.
#[pyfunction]
#[pyo3(signature = (*, sql, start_sql=None, end_sql=None))]
pub(crate) fn resolve_cursor_intrinsics(
    py: Python<'_>,
    sql: &str,
    start_sql: Option<&str>,
    end_sql: Option<&str>,
) -> PyResult<(String, bool)> {
    let (rendered, found) = replaced(
        py,
        sql,
        SQL_CONTEXT,
        (
            start_sql.unwrap_or(CURSOR_START_CALL),
            end_sql.unwrap_or(CURSOR_END_CALL),
        ),
    )?;
    if !found || start_sql.is_none() || end_sql.is_none() {
        return Ok((sql.to_owned(), found));
    }
    Ok((rendered, true))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(validated_model_cursor_intrinsics, module)?)?;
    module.add_function(wrap_pyfunction!(cursor_intrinsics_analysis_sql, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_cursor_intrinsics, module)?)?;
    Ok(())
}
