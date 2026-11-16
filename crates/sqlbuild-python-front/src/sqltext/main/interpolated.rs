//! One SQL text interpolated: the SQL (`None` when unchanged), its reads, and its error.

use crate::sqltext::_helpers::static_sql::InterpolatedRow;
use crate::sqltext::_helpers::static_sql::PythonInterpolationHost;
use crate::sqltext::_helpers::static_sql::read_rows;
use crate::sqltext::models::InterpolationSources;
use pyo3::prelude::PyResult;
use sqlbuild_sqltext::compiler::main::sql_interpolation::interpolate_sql;
use std::cell::RefCell;

/// One SQL text interpolated: the SQL (`None` when unchanged), its reads, and its error.
pub fn interpolated(
    sources: InterpolationSources<'_>,
    sql: &str,
    file_path: &str,
) -> PyResult<InterpolatedRow> {
    let host = PythonInterpolationHost {
        sources,
        raised: RefCell::new(None),
    };
    let row = match interpolate_sql(&host, sql, file_path) {
        Ok(result) => (result.sql, read_rows(result.reads), None),
        Err(failure) => (None, read_rows(failure.reads), Some(failure.message)),
    };
    if let Some(error) = host.raised.borrow_mut().take() {
        return Err(error);
    }
    Ok(row)
}
