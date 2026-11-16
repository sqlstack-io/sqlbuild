//! The model's `@@@name` placeholder defaults as strings, or `None`.

use crate::project_assembly::_helpers::syntax_checks::config_value;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods};

/// The model's `@@@name` placeholder defaults as strings, or `None`.
#[pyfunction]
pub fn model_placeholders<'py>(
    model_input: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyDict>>> {
    let raw: Bound<'py, PyAny> = config_value(model_input, "placeholders")?;
    let Ok(raw) = raw.downcast::<PyDict>() else {
        return Ok(None);
    };
    let placeholders: Bound<'py, PyDict> = PyDict::new(model_input.py());
    for (key, value) in raw.iter() {
        placeholders.set_item(key.str()?, value.str()?)?;
    }
    Ok(Some(placeholders))
}
