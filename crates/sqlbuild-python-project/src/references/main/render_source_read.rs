//! A source as the table factor that reads it, deduplicated with `FINAL` when `final`.

use crate::references::_helpers::source_relations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A source as the table factor that reads it, deduplicated with `FINAL` when `final`.
#[inline]
pub fn render_source_read<'py>(
    entry: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<String> {
    source_relations::render_source_read(entry, adapter)
}
