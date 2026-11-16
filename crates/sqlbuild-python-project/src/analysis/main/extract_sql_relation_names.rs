//! `(relations, temporary names)` named in `sql`, or None if unparseable.

use crate::analysis::_helpers::relation_names;
use crate::analysis::types::RelationRow;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// `(relations, temporary names)` named in `sql`, or None if unparseable.
#[inline]
pub fn extract_sql_relation_names(
    py: Python<'_>,
    sql: &str,
    dialect: Option<&str>,
) -> PyResult<Option<(Vec<RelationRow>, Vec<String>)>> {
    relation_names::extract_sql_relation_names(py, sql, dialect)
}
