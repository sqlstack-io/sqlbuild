//! Each `(sql, file path)` interpolated in order; non-rendering Python failures raise.

use crate::sqltext::_helpers::static_sql;
use crate::sqltext::models::InterpolationRow;
use crate::sqltext::models::InterpolationSources;
use pyo3::prelude::PyResult;

/// Each `(sql, file path)` interpolated in order; non-rendering Python failures raise.
#[inline]
pub fn interpolation_batch(
    sources: InterpolationSources<'_>,
    sqls: &[(String, String)],
) -> PyResult<Vec<InterpolationRow>> {
    static_sql::interpolation_batch(sources, sqls)
}
