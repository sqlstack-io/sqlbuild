//! A normalization rejection as Python's `ValueError`; anything else as an internal failure.

use crate::analysis_session::_helpers::query_columns;
use pyo3::PyErr;
use sqlbuild_analysis::assembly::analysis_session::errors::QueryColumnsFailure;

/// A normalization rejection as Python's `ValueError`; anything else as an internal failure.
#[inline]
pub fn query_columns_error(failure: QueryColumnsFailure) -> PyErr {
    query_columns::query_columns_error(failure)
}
