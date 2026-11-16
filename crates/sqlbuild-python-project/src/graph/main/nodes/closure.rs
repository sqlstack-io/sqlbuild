//! Keys within `max_depth` edges of any start, plus the starts when `include_starts`.

use crate::graph::_helpers::nodes::algorithms::neighbours;
use crate::graph::models::PyNodeSet;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::{PySet, PySetMethods, PyTuple};
use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;

/// Keys within `max_depth` edges of any start, plus the starts when `include_starts`.
pub fn closure<'py>(
    starts: Vec<Bound<'py, PyAny>>,
    edges: &Bound<'py, PyAny>,
    (include_starts, max_depth): (bool, Option<i64>),
) -> PyResult<Bound<'py, PySet>> {
    let py: Python<'py> = edges.py();
    let mut visited: PyNodeSet<'py> = PyNodeSet(PySet::empty(py)?);
    if include_starts {
        for start in &starts {
            visited.0.add(start)?;
        }
    }
    let none: Bound<'py, PyTuple> = PyTuple::empty(py);
    breadth_first_reach(
        starts,
        max_depth.map(|maximum| u64::try_from(maximum).unwrap_or(0)),
        &mut visited,
        |current| neighbours(edges, current, &none),
    )?;
    Ok(visited.0)
}
