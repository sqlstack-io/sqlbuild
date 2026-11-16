//! Render one validated constant with the active adapter's typed-value contract.

use crate::render::_helpers::declarations::declaration_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Render one validated constant with the active adapter's typed-value contract.
#[inline]
pub fn constant_declaration_sql<'py>(
    declaration: &Bound<'py, PyAny>,
    value_renderer: &Bound<'py, PyAny>,
    (collection_rendering, file_path): (&Bound<'py, PyAny>, Option<&str>),
) -> PyResult<String> {
    declaration_values::constant_declaration_sql(
        declaration,
        value_renderer,
        (collection_rendering, file_path),
    )
}
