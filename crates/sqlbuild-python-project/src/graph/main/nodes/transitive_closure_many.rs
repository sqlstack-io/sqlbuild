//! Keys within `max_depth` edges of any start, plus the starts when requested.

use crate::graph::main::nodes::closure::closure;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyFrozenSet, PySet, PySetMethods};

/// Keys within `max_depth` edges of any start, plus the starts when requested.
#[pyfunction]
#[pyo3(signature = (*, starts, edges, include_starts, max_depth = None))]
pub fn transitive_closure_many<'py>(
    starts: &Bound<'py, PyAny>,
    edges: &Bound<'py, PyAny>,
    include_starts: bool,
    max_depth: Option<i64>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let starts: Vec<Bound<'py, PyAny>> = starts.try_iter()?.collect::<PyResult<_>>()?;
    let visited: Bound<'py, PySet> = closure(starts, edges, (include_starts, max_depth))?;
    PyFrozenSet::new(edges.py(), visited.iter())
}
