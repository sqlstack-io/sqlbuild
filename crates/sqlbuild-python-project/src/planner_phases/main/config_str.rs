//! A model config value as text, `None` when absent or not text.

use pyo3::prelude::{Bound, PyAny, PyResult};
use sqlbuild_python_front::model_config::main::config::config_text::config_text;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

/// A model config value as text, `None` when absent or not text.
pub fn config_str(model: &Bound<'_, PyAny>, key: &str) -> PyResult<Option<String>> {
    config_text(model.py(), &model_values(model)?, key)
}
