//! Attach the built tree to its public record, rather than reconstructing it from that view.

use crate::model_config::_helpers::config::typed_config;
use crate::model_config::types::BuiltConfigValues;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Attach the built tree to its public record, rather than reconstructing it from that view.
#[inline]
pub fn retain_values(config: &Bound<'_, PyAny>, built: Option<&BuiltConfigValues>) -> PyResult<()> {
    typed_config::retain_values(config, built)
}
