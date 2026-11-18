//! `(relations, temporary names)` named in `sql`, or None if unparseable.

use crate::analysis::_helpers::relation_names::{
    EXTRACTED, relation_names_error, remember, remembered, rows_of,
};
use crate::analysis::types::ExtractedRows;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::relation_names::main::extract_relation_names::extract_relation_names;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// `(relations, temporary names)` named in `sql`, or None if unparseable.
pub fn extract_sql_relation_names(
    py: Python<'_>,
    sql: &str,
    dialect: Option<&str>,
) -> PyResult<ExtractedRows> {
    if let Some(rows) = remembered(&EXTRACTED, dialect, sql) {
        return Ok(rows);
    }
    let extracted = py
        .compiler_detach(|| Ok(extract_relation_names(sql, dialect)))
        .map_err(compiler_error)?
        .map_err(relation_names_error)?;
    let rows: ExtractedRows = rows_of(extracted);
    remember(&EXTRACTED, dialect, [(sql.to_owned(), rows.clone())]);
    Ok(rows)
}
