//! Configure the active exporter scope from validated discovery inputs.

use crate::event_exporting::_helpers::scope;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Configure the active exporter scope from validated discovery inputs.
#[inline]
pub fn configure_discovered_event_exporters(discovered: &Bound<'_, PyAny>) -> PyResult<()> {
    scope::configure_discovered_event_exporters(discovered)
}
