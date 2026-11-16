//! Each `(sql, file path)` interpolated in order; non-rendering Python failures raise.

use crate::sqltext::_helpers::static_sql::PythonInterpolationHost;
use crate::sqltext::_helpers::static_sql::read_rows;
use crate::sqltext::models::InterpolationRow;
use crate::sqltext::models::InterpolationSources;
use pyo3::prelude::PyResult;
use sqlbuild_sqltext::compiler::main::sql_interpolation::interpolate_sql;
use std::cell::RefCell;

/// Each `(sql, file path)` interpolated in order; non-rendering Python failures raise.
pub fn interpolation_batch(
    sources: InterpolationSources<'_>,
    sqls: &[(String, String)],
) -> PyResult<Vec<InterpolationRow>> {
    let host = PythonInterpolationHost {
        sources,
        raised: RefCell::new(None),
    };
    let mut rows = Vec::with_capacity(sqls.len());
    for (sql, file_path) in sqls {
        let row = match interpolate_sql(&host, sql, file_path) {
            Ok(result) => InterpolationRow {
                sql: result.sql,
                spans: result.spans,
                reads: read_rows(result.reads),
                error: None,
            },
            Err(failure) => InterpolationRow {
                sql: None,
                spans: Vec::new(),
                reads: read_rows(failure.reads),
                error: Some(failure.message),
            },
        };
        if let Some(error) = host.raised.borrow_mut().take() {
            return Err(error);
        }
        rows.push(row);
    }
    Ok(rows)
}
