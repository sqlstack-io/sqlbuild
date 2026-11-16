//! Normalize analysis sql.

use crate::analysis::_helpers::normalization;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Normalize analysis sql.
#[inline]
pub fn normalize_analysis_sql(
    py: Python<'_>,
    request: crate::analysis::models::NormalizationInput,
) -> PyResult<String> {
    normalization::normalize_analysis_sql(py, request)
}
