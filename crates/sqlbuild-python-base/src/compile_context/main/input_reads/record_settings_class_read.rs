//! Note a provider settings class instantiated from the environment and files.

use crate::compile_context::_helpers::input_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Note a provider settings class instantiated from the environment and files.
#[inline]
pub fn record_settings_class_read(settings_class: &Bound<'_, PyAny>) -> PyResult<()> {
    input_reads::record_settings_class_read(settings_class)
}
