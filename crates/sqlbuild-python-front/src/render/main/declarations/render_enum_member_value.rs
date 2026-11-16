//! One enum member value as a SQL scalar literal.

use crate::render::_helpers::declarations::declaration_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// One enum member value as a SQL scalar literal.
#[inline]
pub fn render_enum_member_value(value: &Bound<'_, PyAny>) -> PyResult<String> {
    declaration_values::render_enum_member_value(value)
}
