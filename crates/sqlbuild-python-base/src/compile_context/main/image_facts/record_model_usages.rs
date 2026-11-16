//! Record the digest of one model's scope usages, which decide declaration placement.

use crate::compile_context::_helpers::image_facts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Record the digest of one model's scope usages, which decide declaration placement.
#[inline]
pub fn record_model_usages(
    model: &Bound<'_, PyAny>,
    usages: (&[Bound<'_, PyAny>], &[Bound<'_, PyAny>]),
) -> PyResult<()> {
    image_facts::record_model_usages(model, usages)
}
