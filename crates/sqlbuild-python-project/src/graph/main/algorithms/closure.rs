//! Keys within `max_depth` edges of any start, plus the starts when `include_starts`.

use crate::graph::_helpers::algorithms;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PySet;

/// Keys within `max_depth` edges of any start, plus the starts when `include_starts`.
#[inline]
pub fn closure<'py>(
    starts: Vec<Bound<'py, PyAny>>,
    edges: &Bound<'py, PyAny>,
    (include_starts, max_depth): (bool, Option<i64>),
) -> PyResult<Bound<'py, PySet>> {
    algorithms::closure(starts, edges, (include_starts, max_depth))
}
