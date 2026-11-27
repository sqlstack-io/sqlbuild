//! Cursor intrinsic validation, rejection and rendering.

use crate::attachments::constants::MICROBATCH_END_SENTINEL;
use crate::attachments::constants::MICROBATCH_START_SENTINEL;
use crate::attachments::main::analysis_cursor_sql::analysis_cursor_sql;
use crate::attachments::main::compile_input_error::compile_input_error;
use crate::attachments::main::validated_model_cursor_sql::validated_model_cursor_sql;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::{PyString, PyStringMethods};
use pyo3::{Python, pyfunction};
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics::replace_intrinsics;
use sqlbuild_sqltext::sql_scan::main::bounded_expansions::bounded_expansions;

use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;

pub(crate) const INCREMENTAL_MATERIALIZATION: &str = "incremental";
const CURSOR_START: &str = "__cursor_start";
pub(crate) const CURSOR_START_CALL: &str = "__cursor_start()";
pub(crate) const CURSOR_END_CALL: &str = "__cursor_end()";
const INTEGER_CURSOR_TYPE: &str = "integer";
const INTEGER_ANALYSIS_BOUND: &str = "CAST(0 AS BIGINT)";
const TIMESTAMP_ANALYSIS_BOUND: &str = "CAST('2000-01-01 00:00:00' AS TIMESTAMP)";
pub(crate) const MODEL_SQL_CONTEXT: &str = "Model SQL";
pub(crate) const SQL_CONTEXT: &str = "SQL";

pub(crate) const RESERVED_CURSOR_MARKERS: [&str; 2] =
    [MICROBATCH_START_SENTINEL, MICROBATCH_END_SENTINEL];

/// `text` when it is a string, else `None`.
pub(crate) fn text_value(value: &Bound<'_, PyAny>) -> Option<String> {
    match value.cast::<PyString>() {
        Ok(text) => Some(text.to_string_lossy().into_owned()),
        Err(_) => None,
    }
}

/// `sql` with the cursor markers replaced by concrete bounds, none joining a neighbour into a comment.
#[pyfunction]
#[pyo3(signature = (*, sql, start_sql, end_sql))]
pub(crate) fn replace_cursor_markers(sql: &str, start_sql: &str, end_sql: &str) -> String {
    let mut sites: Vec<(usize, usize, &str)> = Vec::new();
    for (marker, value) in RESERVED_CURSOR_MARKERS
        .into_iter()
        .zip([start_sql, end_sql])
    {
        sites.extend(
            sql.match_indices(marker)
                .map(|(position, found)| (position, position + found.len(), value)),
        );
    }
    sites.sort_unstable_by_key(|(position, _, _)| *position);
    let mut texts: Vec<String> = sites
        .iter()
        .map(|(_, _, value)| (*value).to_owned())
        .collect();
    for bounded in bounded_expansions(sql, &sites, None) {
        texts[bounded.index] = bounded.text;
    }
    let mut replaced: String = String::with_capacity(sql.len());
    let mut copied: usize = 0;
    for ((position, after, _), text) in sites.iter().zip(&texts) {
        replaced.push_str(&sql[copied..*position]);
        replaced.push_str(text);
        copied = *after;
    }
    replaced.push_str(&sql[copied..]);
    replaced
}

/// `sql` with each intrinsic replaced by its bound and whether any was, raising for `context`.
pub(crate) fn replaced(
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

/// The literal static analysis reads for every cursor intrinsic of a model with `cursor_type`.
pub(crate) fn analysis_bound(cursor_type: Option<&str>) -> &'static str {
    if cursor_type == Some(INTEGER_CURSOR_TYPE) {
        INTEGER_ANALYSIS_BOUND
    } else {
        TIMESTAMP_ANALYSIS_BOUND
    }
}

/// One model query with canonical intrinsic calls, raising `CompileInputError` when invalid.
#[pyfunction]
#[pyo3(signature = (*, sql, config_values, model_name))]
pub(crate) fn validated_model_cursor_intrinsics(
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
pub(crate) fn cursor_intrinsics_analysis_sql(
    py: Python<'_>,
    sql: &str,
    cursor_type: &Bound<'_, PyAny>,
) -> PyResult<String> {
    analysis_cursor_sql(py, sql, cursor_type)
}
