//! Optional facts.

use crate::plan_output::_helpers::display::label_facts;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::plan_display::models::ModelLabelFacts;

/// Optional facts.
pub fn optional_facts(entry: &Bound<'_, PyAny>) -> PyResult<Option<ModelLabelFacts>> {
    if entry.is_none() {
        return Ok(None);
    }
    label_facts(entry).map(Some)
}
