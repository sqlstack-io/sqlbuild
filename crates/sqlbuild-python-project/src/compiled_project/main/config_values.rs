//! The config values, or the encoder's error, kept for the consumer that reads them.

use crate::compiled_project::_helpers::project;
use crate::compiled_project::_helpers::project::ConfigValues;
use pyo3::prelude::Bound;
use pyo3::types::PyDict;

/// The config values, or the encoder's error, kept for the consumer that reads them.
#[inline]
pub fn config_values(values: &Bound<'_, PyDict>) -> ConfigValues {
    project::config_values(values)
}
