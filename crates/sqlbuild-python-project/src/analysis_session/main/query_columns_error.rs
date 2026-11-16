//! A normalization rejection as Python's `ValueError`; anything else as an internal failure.

use crate::analysis_session::_helpers::query_columns::QUERY_COLUMNS_CONTEXT;
use pyo3::PyErr;
use sqlbuild_analysis::assembly::analysis_session::errors::QueryColumnsFailure;
use sqlbuild_core::panics::main::native_failure::native_failure;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;

/// A normalization rejection as Python's `ValueError`; anything else as an internal failure.
pub fn query_columns_error(failure: QueryColumnsFailure) -> PyErr {
    match failure {
        QueryColumnsFailure::Normalization(message) => compiler_error(message),
        QueryColumnsFailure::Internal(reason) => {
            compiler_error(native_failure(QUERY_COLUMNS_CONTEXT, &reason))
        }
    }
}
