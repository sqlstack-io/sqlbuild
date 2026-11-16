//! The project's lineage edges and selector indexes, from its native graph.

use crate::graph::_helpers::views;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The project's lineage edges and selector indexes, from its native graph.
#[inline]
pub fn project_lineage_views<'py>(
    py: Python<'py>,
    project: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    views::project_lineage_views(py, project)
}
