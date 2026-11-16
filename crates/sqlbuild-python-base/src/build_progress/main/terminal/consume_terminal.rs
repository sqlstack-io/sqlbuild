//! The projector's canonical duration of a finished resource, `None` while it is pending.

use crate::build_progress::_helpers::terminal;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The projector's canonical duration of a finished resource, `None` while it is pending.
#[inline]
pub fn consume_terminal(
    projector: &Bound<'_, PyAny>,
    resource_name: &str,
    resource_id: Option<&str>,
) -> PyResult<Option<f64>> {
    terminal::consume_terminal(projector, resource_name, resource_id)
}
