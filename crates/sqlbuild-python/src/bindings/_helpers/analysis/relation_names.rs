//! Relations named in SQL and temporary tables it creates, for hard-coded name checks.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use pyo3::PyErr;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::relation_names::errors::RelationNamesFailure;
use sqlbuild_analysis::relation_names::main::extract_relation_names::extract_relation_names;
use sqlbuild_analysis::relation_names::models::{ExtractedRelations, RelationName};
use sqlbuild_core::panics::main::native_failure::native_failure;

use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::input_loops::model_loop_prepass::pooled;
use crate::bindings::types::CompilerDetach;

const RELATION_NAMES_CONTEXT: &str = "native relation name extraction";
/// Distinct SQL texts the memo keeps before starting over; a 10K-model project holds about 10K.
const EXTRACTED_LIMIT: usize = 100_000;

/// `(name, schema, database)`.
pub(crate) type RelationRow = (String, Option<String>, Option<String>);

/// `(relations, temporary names)` named in a SQL text, or None if unparseable.
pub(crate) type ExtractedRows = Option<(Vec<RelationRow>, Vec<String>)>;

/// Extractions by dialect and SQL text: the text alone decides them, so a location's previews of one
/// publication reuse them.
static EXTRACTED: Mutex<Option<ExtractedMemo>> = Mutex::new(None);

/// Extracted rows by `(dialect, SQL text)`.
type ExtractedMemo = HashMap<(Option<String>, String), ExtractedRows>;

fn remembered(dialect: Option<&str>, sql: &str) -> Option<ExtractedRows> {
    let memo = EXTRACTED.lock().unwrap_or_else(PoisonError::into_inner);
    memo.as_ref()?
        .get(&(dialect.map(str::to_owned), sql.to_owned()))
        .cloned()
}

fn remember(dialect: Option<&str>, extracted: impl IntoIterator<Item = (String, ExtractedRows)>) {
    let mut memo = EXTRACTED.lock().unwrap_or_else(PoisonError::into_inner);
    let entries = memo.get_or_insert_with(HashMap::new);
    if entries.len() > EXTRACTED_LIMIT {
        entries.clear();
    }
    for (sql, rows) in extracted {
        entries.insert((dialect.map(str::to_owned), sql), rows);
    }
}

fn rows_of(extracted: Option<ExtractedRelations>) -> ExtractedRows {
    extracted.map(
        |ExtractedRelations {
             relations,
             temporary,
         }| { (relations.into_iter().map(relation_row).collect(), temporary) },
    )
}

/// `(relations, temporary names)` named in `sql`, or None if unparseable.
pub(crate) fn extract_sql_relation_names(
    py: Python<'_>,
    sql: &str,
    dialect: Option<&str>,
) -> PyResult<ExtractedRows> {
    if let Some(rows) = remembered(dialect, sql) {
        return Ok(rows);
    }
    let extracted = py
        .compiler_detach(|| Ok(extract_relation_names(sql, dialect)))
        .map_err(compiler_error)?
        .map_err(relation_names_error)?;
    let rows: ExtractedRows = rows_of(extracted);
    remember(dialect, [(sql.to_owned(), rows.clone())]);
    Ok(rows)
}

/// Extract every SQL text not extracted yet, in parallel off the GIL, so later reads find them.
pub(crate) fn prefetch_sql_relation_names(
    py: Python<'_>,
    sqls: Vec<String>,
    dialect: Option<&str>,
) -> PyResult<()> {
    let missing: Vec<String> = sqls
        .into_iter()
        .filter(|sql| remembered(dialect, sql).is_none())
        .collect();
    let extracted: Vec<Result<Option<ExtractedRelations>, RelationNamesFailure>> = py
        .compiler_detach(|| {
            Ok(pooled(&missing, |sql: &String| {
                extract_relation_names(sql, dialect)
            }))
        })
        .map_err(compiler_error)?;
    let mut rows: Vec<(String, ExtractedRows)> = Vec::with_capacity(missing.len());
    for (sql, extracted) in missing.into_iter().zip(extracted) {
        rows.push((sql, rows_of(extracted.map_err(relation_names_error)?)));
    }
    remember(dialect, rows);
    Ok(())
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
