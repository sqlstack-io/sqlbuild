//! Closure, edge inversion and path nodes over any Python-keyed edge mapping.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{
    PyDict, PyDictMethods, PyFrozenSet, PyList, PyListMethods, PySet, PySetMethods, PyTuple,
};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;
use sqlbuild_core::graph_walk::main::directed_path_nodes::directed_path_nodes;

use crate::bindings::_helpers::graph::node_set::PyNodeSet;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

const BUILTINS_MODULE: &str = "builtins";

/// Keys within `max_depth` edges of any start, plus the starts when `include_starts`.
pub(crate) fn closure<'py>(
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

/// The keys `edges.get(current, ())` lists.
fn neighbours<'py>(
    edges: &Bound<'py, PyAny>,
    current: &Bound<'py, PyAny>,
    none: &Bound<'py, PyTuple>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    mapping_get_or(edges, current, none)?.try_iter()?.collect()
}

/// Keys within `max_depth` edges of any start, plus the starts when requested.
#[pyfunction]
#[pyo3(signature = (*, starts, edges, include_starts, max_depth = None))]
pub(crate) fn transitive_closure_many<'py>(
    starts: &Bound<'py, PyAny>,
    edges: &Bound<'py, PyAny>,
    include_starts: bool,
    max_depth: Option<i64>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let starts: Vec<Bound<'py, PyAny>> = starts.try_iter()?.collect::<PyResult<_>>()?;
    let visited: Bound<'py, PySet> = closure(starts, edges, (include_starts, max_depth))?;
    PyFrozenSet::new(edges.py(), visited.iter())
}

/// All graph keys reachable from a starting key.
#[pyfunction]
#[pyo3(signature = (*, start, edges, max_depth = None))]
fn transitive_closure<'py>(
    start: Bound<'py, PyAny>,
    edges: &Bound<'py, PyAny>,
    max_depth: Option<i64>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let visited: Bound<'py, PySet> = closure(vec![start], edges, (false, max_depth))?;
    PyFrozenSet::new(edges.py(), visited.iter())
}

/// Reversed edges: every source key first, then each new target in first-seen order.
pub(crate) fn inverted<'py>(edges: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = edges.py();
    let inverted: Bound<'py, PyDict> = PyDict::new(py);
    for key in edges.try_iter()? {
        inverted.set_item(key?, PyList::empty(py))?;
    }
    for item in mapping_items(edges)?.try_iter()? {
        let (source, targets): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item?.extract()?;
        for target in targets.try_iter()? {
            let target: Bound<'py, PyAny> = target?;
            let sources: Bound<'py, PyList> = match inverted.get_item(&target)? {
                Some(sources) => sources.downcast_into()?,
                None => {
                    let sources: Bound<'py, PyList> = PyList::empty(py);
                    inverted.set_item(&target, &sources)?;
                    sources
                }
            };
            sources.append(&source)?;
        }
    }
    Ok(inverted)
}

/// Reversed directed graph edges, each source list sorted by `sort_key` when given.
#[pyfunction]
#[pyo3(signature = (*, edges, sort_key = None))]
fn invert_edges<'py>(
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
fn path_nodes<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(transitive_closure, module)?)?;
    module.add_function(wrap_pyfunction!(transitive_closure_many, module)?)?;
    module.add_function(wrap_pyfunction!(invert_edges, module)?)?;
    module.add_function(wrap_pyfunction!(path_nodes, module)?)?;
    Ok(())
}
