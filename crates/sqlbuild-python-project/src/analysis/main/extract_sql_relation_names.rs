//! `(relations, temporary names)` named in `sql`, or None if unparseable.

use crate::analysis::_helpers::relation_names::relation_names_error;
use crate::analysis::_helpers::relation_names::relation_row;
use crate::analysis::types::RelationRow;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::relation_names::main::extract_relation_names::extract_relation_names;
use sqlbuild_analysis::relation_names::models::ExtractedRelations;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// `(relations, temporary names)` named in `sql`, or None if unparseable.
pub fn extract_sql_relation_names(
    py: Python<'_>,
    sql: &str,
    dialect: Option<&str>,
) -> PyResult<Option<(Vec<RelationRow>, Vec<String>)>> {
    let extracted = py
        .compiler_detach(|| Ok(extract_relation_names(sql, dialect)))
        .map_err(compiler_error)?
        .map_err(relation_names_error)?;
    Ok(extracted.map(
        |ExtractedRelations {
             relations,
             temporary,
         }| { (relations.into_iter().map(relation_row).collect(), temporary) },
    ))
}
