//! The config values, or the encoder's error, kept for the consumer that reads them.

use crate::compiled_project::_helpers::project::ConfigValues;
use crate::compiled_project::_helpers::values::object_value;
use pyo3::prelude::Bound;
use pyo3::types::PyDict;
use serde_json::Value;
use std::collections::BTreeMap;

/// The config values, or the encoder's error, kept for the consumer that reads them.
pub fn config_values(values: &Bound<'_, PyDict>) -> ConfigValues {
    match object_value(values) {
        Ok(Value::Object(entries)) => (entries.into_iter().collect(), None),
        Ok(_) => (BTreeMap::new(), None),
        Err(error) => (BTreeMap::new(), Some(error.to_string())),
    }
}
