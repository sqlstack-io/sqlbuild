//! One string-valued model configuration entry.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_front::model_config::main::config::config_text_object::config_text_object;
use sqlbuild_python_front::model_config::main::config::model_values::model_values;

/// One string-valued model configuration entry.
pub fn config_str<'py>(model: &Bound<'py, PyAny>, key: &str) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    Ok(config_text_object(py, &model_values(model)?, key)?
        .unwrap_or_else(|| py.None().into_bound(py)))
}
