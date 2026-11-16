//! A model's config values as typed nodes, read key by key.

use crate::model_config::_helpers::config::typed_config::CONFIG_FIELD;
use crate::model_config::main::config::typed_values::typed_values;
use crate::model_config::types::ConfigValues;
use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A model's config values as typed nodes, read key by key.
pub fn model_values<'py>(model: &Bound<'py, PyAny>) -> PyResult<ConfigValues<'py>> {
    typed_values(&native_field(model, CONFIG_FIELD)?)
}
