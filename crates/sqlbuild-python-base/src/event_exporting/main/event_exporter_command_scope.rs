//! Install one command-owned exporter scope.

use crate::event_exporting::_helpers::scope;
use crate::event_exporting::models::EventExporterScopeContext;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;

/// Install one command-owned exporter scope.
#[inline]
pub fn event_exporter_command_scope(scope: Py<PyAny>) -> EventExporterScopeContext {
    scope::event_exporter_command_scope(scope)
}
