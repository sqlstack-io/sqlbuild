//! The note and help for turning SQL analysis on, or `None` when it is effectively on.

use crate::discovery::_helpers::failures::guidance;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The note and help for turning SQL analysis on, or `None` when it is effectively on.
#[inline]
pub fn sql_analysis_guidance(
    discovered_inputs: &Bound<'_, PyAny>,
    purpose: &str,
) -> PyResult<Option<(String, String)>> {
    guidance::sql_analysis_guidance(discovered_inputs, purpose)
}
