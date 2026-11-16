//! Relations named in SQL and temporary tables it creates, for hard-coded name checks.

use pyo3::PyErr;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::relation_names::errors::RelationNamesFailure;
use sqlbuild_analysis::relation_names::main::extract_relation_names::extract_relation_names;
use sqlbuild_analysis::relation_names::models::{ExtractedRelations, RelationName};
use sqlbuild_core::panics::main::native_failure::native_failure;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

const RELATION_NAMES_CONTEXT: &str = "native relation name extraction";

/// `(name, schema, database)`.
pub(crate) type RelationRow = (String, Option<String>, Option<String>);

/// `(relations, temporary names)` named in `sql`, or None if unparseable.
pub(crate) fn extract_sql_relation_names(
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

fn relation_row(relation: RelationName) -> RelationRow {
    (relation.name, relation.schema, relation.database)
}

/// An unknown dialect as the wheel's `ValueError`; anything else as an internal failure.
fn relation_names_error(failure: RelationNamesFailure) -> PyErr {
    match failure {
        RelationNamesFailure::UnknownDialect(message) => compiler_error(message),
        RelationNamesFailure::Internal(reason) => {
            compiler_error(native_failure(RELATION_NAMES_CONTEXT, &reason))
        }
    }
}
