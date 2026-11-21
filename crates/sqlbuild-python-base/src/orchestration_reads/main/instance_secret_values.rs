//! The secret values provider instances hold, however their settings were loaded, such as from a `.env` file.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyBytes, PyBytesMethods, PyString};

/// Each nonempty value of a secret setting (one with `get_secret_value()`) of `instances`.
pub fn instance_secret_values(instances: &[Bound<'_, PyAny>]) -> PyResult<Vec<String>> {
    let mut values: Vec<String> = Vec::new();
    for instance in instances {
        for name in instance.get_type().getattr("model_fields")?.try_iter()? {
            let value = instance.getattr(name?.downcast_into::<PyString>()?)?;
            if !value.hasattr("get_secret_value")? {
                continue;
            }
            let secret = value.call_method0("get_secret_value")?;
            let text = match secret.downcast::<PyBytes>() {
                Ok(bytes) => String::from_utf8_lossy(bytes.as_bytes()).into_owned(),
                Err(_) => secret.str()?.extract::<String>()?,
            };
            if !text.is_empty() {
                values.push(text);
            }
        }
    }
    Ok(values)
}
