//! Python's exception for a type that cannot be normalized.

use crate::type_system::_helpers::normalization;
use pyo3::prelude::PyErr;
use sqlbuild_analysis::type_system::errors::TypeNormalizationError;

/// Python's exception for a type that cannot be normalized.
#[inline]
pub fn normalization_error(error: &TypeNormalizationError) -> PyErr {
    normalization::normalization_error(error)
}
