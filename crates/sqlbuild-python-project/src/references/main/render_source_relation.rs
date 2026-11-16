//! Render a source as a SQL table factor, through the adapter when one is given.

use crate::references::_helpers::source_relations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Render a source as a SQL table factor, through the adapter when one is given.
#[inline]
pub fn render_source_relation<'py>(
    entry: &Bound<'py, PyAny>,
    adapter: Option<&Bound<'py, PyAny>>,
) -> PyResult<String> {
    source_relations::render_source_relation(entry, adapter)
}
