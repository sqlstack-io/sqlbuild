//! The providers the project's lifecycle and command-output sinks use, which run with every command.

use crate::discovery::_helpers::sources::provider_secrets;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The providers the project's lifecycle and command-output sinks use, which run with every command.
#[inline]
pub fn sink_uses(discovered: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    provider_secrets::sink_uses(discovered)
}
