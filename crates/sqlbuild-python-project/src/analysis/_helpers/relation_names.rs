//! Relations named in SQL and temporary tables it creates, for hard-coded name checks.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use crate::analysis::types::{ExtractedRows, RelationRow};
use pyo3::PyErr;
use sqlbuild_analysis::relation_names::errors::RelationNamesFailure;
use sqlbuild_analysis::relation_names::models::{ExtractedRelations, RelationName};
use sqlbuild_core::panics::main::native_failure::native_failure;

use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;

const RELATION_NAMES_CONTEXT: &str = "native relation name extraction";
/// Distinct SQL texts the memo keeps before starting over; a 10K-model project holds about 10K.
const EXTRACTED_LIMIT: usize = 100_000;

/// Extractions by dialect and SQL text, reused by a location's previews of one publication.
pub(crate) static EXTRACTED: Mutex<Option<ExtractedMemo>> = Mutex::new(None);

/// Extracted rows by `(dialect, SQL text)`.
pub(crate) type ExtractedMemo = HashMap<(Option<String>, String), ExtractedRows>;

pub(crate) fn remembered(
    memo: &Mutex<Option<ExtractedMemo>>,
    dialect: Option<&str>,
    sql: &str,
) -> Option<ExtractedRows> {
    let memo = memo.lock().unwrap_or_else(PoisonError::into_inner);
    memo.as_ref()?
        .get(&(dialect.map(str::to_owned), sql.to_owned()))
        .cloned()
}

pub(crate) fn remember(
    memo: &Mutex<Option<ExtractedMemo>>,
    dialect: Option<&str>,
    extracted: impl IntoIterator<Item = (String, ExtractedRows)>,
) {
    let mut memo = memo.lock().unwrap_or_else(PoisonError::into_inner);
    let entries = memo.get_or_insert_with(HashMap::new);
    if entries.len() > EXTRACTED_LIMIT {
        entries.clear();
    }
    for (sql, rows) in extracted {
        entries.insert((dialect.map(str::to_owned), sql), rows);
    }
}

pub(crate) fn rows_of(extracted: Option<ExtractedRelations>) -> ExtractedRows {
    extracted.map(
        |ExtractedRelations {
             relations,
             temporary,
         }| { (relations.into_iter().map(relation_row).collect(), temporary) },
    )
}

pub(crate) fn relation_row(relation: RelationName) -> RelationRow {
    (relation.name, relation.schema, relation.database)
}

/// An unknown dialect as the wheel's `ValueError`; anything else as an internal failure.
pub(crate) fn relation_names_error(failure: RelationNamesFailure) -> PyErr {
    match failure {
        RelationNamesFailure::UnknownDialect(message) => compiler_error(message),
        RelationNamesFailure::Internal(reason) => {
            compiler_error(native_failure(RELATION_NAMES_CONTEXT, &reason))
        }
    }
}
