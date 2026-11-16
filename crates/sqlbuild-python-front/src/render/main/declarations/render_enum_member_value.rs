//! One enum member value as a SQL scalar literal.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::PyInt;

/// One enum member value as a SQL scalar literal.
#[pyfunction]
#[pyo3(signature = (*, value))]
pub fn render_enum_member_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    if value.is_instance_of::<PyInt>() {
        return value.str()?.extract();
    }
    let text: String = value.extract()?;
    Ok(format!("'{}'", text.replace('\'', "''")))
}
