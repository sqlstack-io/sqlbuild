//! Closure, edge inversion and path nodes over any Python-keyed edge mapping.

use crate::graph::main::nodes::closure::closure;
use crate::graph::main::nodes::inverted::inverted;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PySet, PySetMethods, PyTuple};
use sqlbuild_core::graph_walk::main::directed_path_nodes::directed_path_nodes;

use crate::graph::models::PyNodeSet;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;

const BUILTINS_MODULE: &str = "builtins";

/// The keys `edges.get(current, ())` lists.
pub(crate) fn neighbours<'py>(
    edges: &Bound<'py, PyAny>,
    current: &Bound<'py, PyAny>,
    none: &Bound<'py, PyTuple>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    mapping_get_or(edges, current, none)?.try_iter()?.collect()
}

/// All graph keys reachable from a starting key.
#[pyfunction]
#[pyo3(signature = (*, start, edges, max_depth = None))]
pub(crate) fn transitive_closure<'py>(
    start: Bound<'py, PyAny>,
    edges: &Bound<'py, PyAny>,
    max_depth: Option<i64>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let visited: Bound<'py, PySet> = closure(vec![start], edges, (false, max_depth))?;
    PyFrozenSet::new(edges.py(), visited.iter())
}

/// Reversed directed graph edges, each source list sorted by `sort_key` when given.
#[pyfunction]
#[pyo3(signature = (*, edges, sort_key = None))]
pub(crate) fn invert_edges<'py>(
    edges: &Bound<'py, PyAny>,
    sort_key: Option<&Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = edges.py();
    let inverted: Bound<'py, PyDict> = inverted(edges)?;
    let sorted: Bound<'py, PyAny> = py.import(BUILTINS_MODULE)?.getattr("sorted")?;
    let result: Bound<'py, PyDict> = PyDict::new(py);
    for (key, sources) in inverted.iter() {
        let sources: Bound<'py, PyAny> = match sort_key {
            None => sources,
            Some(sort_key) => {
                let keywords: Bound<'py, PyDict> = PyDict::new(py);
                keywords.set_item("key", sort_key)?;
                sorted.call((sources,), Some(&keywords))?
            }
        };
        result.set_item(
            key,
            PyTuple::new(py, sources.try_iter()?.collect::<PyResult<Vec<_>>>()?)?,
        )?;
    }
    Ok(result)
}

/// Keys on directed paths between two endpoints, or `None` when `end` is not downstream.
#[pyfunction]
#[pyo3(signature = (*, start, end, downstream))]
pub(crate) fn path_nodes<'py>(
    start: Bound<'py, PyAny>,
    end: Bound<'py, PyAny>,
    downstream: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyFrozenSet>>> {
    let py: Python<'py> = downstream.py();
    let upstream: Bound<'py, PyDict> = inverted(downstream)?;
    let none: Bound<'py, PyTuple> = PyTuple::empty(py);
    let mut reachable: PyNodeSet<'py> = PyNodeSet(PySet::empty(py)?);
    let mut path: PyNodeSet<'py> = PyNodeSet(PySet::empty(py)?);
    let found: bool = directed_path_nodes(
        (start, end),
        (
            |current: &Bound<'py, PyAny>| neighbours(downstream, current, &none),
            |current: &Bound<'py, PyAny>| neighbours(upstream.as_any(), current, &none),
        ),
        (&mut reachable, &mut path),
    )?;
    if !found {
        return Ok(None);
    }
    Ok(Some(PyFrozenSet::new(py, path.0.iter())?))
}
