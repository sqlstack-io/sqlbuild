//! The compiled model of `model_input` from its session outcome, fallback rows and proof.

use crate::hand_built::_helpers::compiled_model;
use crate::hand_built::_helpers::compiled_model::Object;
use pyo3::prelude::PyResult;

/// The compiled model of `model_input` from its session outcome, fallback rows and proof.
#[inline]
pub fn compiled_model<'py>(
    model_input: &Object<'py>,
    deps: &Object<'py>,
    analysis: &Object<'py>,
    macro_deps: &Object<'py>,
) -> PyResult<Object<'py>> {
    compiled_model::compiled_model(model_input, deps, analysis, macro_deps)
}
