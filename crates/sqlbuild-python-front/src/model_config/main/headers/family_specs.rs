//! A Python-authored family contract crosses into native ownership once.

use crate::model_config::_helpers::headers::header_families;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_model_config::header_metadata::models::DynamicFamily;

/// A Python-authored family contract crosses into native ownership once.
#[inline]
pub fn family_specs(families: &Bound<'_, PyAny>) -> PyResult<Vec<DynamicFamily>> {
    header_families::family_specs(families)
}
