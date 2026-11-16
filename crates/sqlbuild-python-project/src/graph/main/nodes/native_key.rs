//! The native `(resource type, name)` pair of a `CompiledObjectKey`.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The native `(resource type, name)` pair of a `CompiledObjectKey`.
pub fn native_key(key: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    Ok((
        native_field(key, "resource_type")?.str()?.extract()?,
        native_field(key, "name")?.extract()?,
    ))
}
