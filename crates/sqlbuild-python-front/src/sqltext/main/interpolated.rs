//! One SQL text interpolated: the SQL (`None` when unchanged), its reads, and its error.

use crate::sqltext::_helpers::static_sql;
use crate::sqltext::_helpers::static_sql::InterpolatedRow;
use crate::sqltext::models::InterpolationSources;
use pyo3::prelude::PyResult;

/// One SQL text interpolated: the SQL (`None` when unchanged), its reads, and its error.
#[inline]
pub fn interpolated(
    sources: InterpolationSources<'_>,
    sql: &str,
    file_path: &str,
) -> PyResult<InterpolatedRow> {
    static_sql::interpolated(sources, sql, file_path)
}
