//! Keys within `max_depth` edges of any start, plus the starts when requested.

use crate::graph::_helpers::algorithms;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::types::PyFrozenSet;

/// Keys within `max_depth` edges of any start, plus the starts when requested.
#[inline]
pub fn transitive_closure_many<'py>(
    starts: &Bound<'py, PyAny>,
    edges: &Bound<'py, PyAny>,
    include_starts: bool,
    max_depth: Option<i64>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    algorithms::transitive_closure_many(starts, edges, include_starts, max_depth)
}
