//! Extract every SQL text not extracted yet, off the GIL, so later reads find them.

use crate::analysis::_helpers::relation_names::{
    EXTRACTED, relation_names_error, remember, remembered, rows_of,
};
use crate::analysis::types::ExtractedRows;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::relation_names::errors::RelationNamesFailure;
use sqlbuild_analysis::relation_names::models::ExtractedRelations;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// Extract every SQL text not extracted yet, in parallel off the GIL, so later reads find them.
pub fn prefetch_sql_relation_names(
    py: Python<'_>,
    sqls: Vec<String>,
    dialect: Option<&str>,
    extract_all: impl FnOnce(&[String]) -> Vec<Result<Option<ExtractedRelations>, RelationNamesFailure>>
    + Send,
) -> PyResult<()> {
    let missing: Vec<String> = sqls
        .into_iter()
        .filter(|sql| remembered(&EXTRACTED, dialect, sql).is_none())
        .collect();
    let extracted: Vec<Result<Option<ExtractedRelations>, RelationNamesFailure>> = py
        .compiler_detach(|| Ok(extract_all(&missing)))
        .map_err(compiler_error)?;
    let mut rows: Vec<(String, ExtractedRows)> = Vec::with_capacity(missing.len());
    for (sql, extracted) in missing.into_iter().zip(extracted) {
        rows.push((sql, rows_of(extracted.map_err(relation_names_error)?)));
    }
    remember(&EXTRACTED, dialect, rows);
    Ok(())
}
