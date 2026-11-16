//! The native `(resource type, name)` pair of a `CompiledObjectKey`.

use crate::graph::_helpers::nodes::keys;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The native `(resource type, name)` pair of a `CompiledObjectKey`.
#[inline]
pub fn native_key(key: &Bound<'_, PyAny>) -> PyResult<(String, String)> {
    keys::native_key(key)
}
