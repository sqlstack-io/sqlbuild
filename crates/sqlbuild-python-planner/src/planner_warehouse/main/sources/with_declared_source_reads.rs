//! Add declared project sources that no selected SQL resource reads to a source map.

use crate::planner_warehouse::_helpers::sources::source_deferral;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyDict;

/// Add declared project sources that no selected SQL resource reads to a source map.
#[inline]
pub fn with_declared_source_reads<'py>(
    project: &Bound<'py, PyAny>,
    source_map: &Bound<'py, PyDict>,
    source_names: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    source_deferral::with_declared_source_reads(project, source_map, source_names)
}
