//! Optional facts.

use crate::plan_output::_helpers::display;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_core::plan_display::models::ModelLabelFacts;

/// Optional facts.
#[inline]
pub fn optional_facts(entry: &Bound<'_, PyAny>) -> PyResult<Option<ModelLabelFacts>> {
    display::optional_facts(entry)
}
