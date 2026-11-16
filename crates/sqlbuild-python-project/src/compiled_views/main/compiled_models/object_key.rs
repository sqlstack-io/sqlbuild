//! Object key.

use crate::compiled_views::_helpers::compiled_models::compiled_model_view;
use crate::compiled_views::models::CompiledViewClasses;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Object key.
#[inline]
pub fn object_key(
    classes: &CompiledViewClasses<'_>,
    kind: &str,
    name: &str,
) -> PyResult<Py<PyAny>> {
    compiled_model_view::object_key(classes, kind, name)
}
