//! The upstream key closure of one compiled graph key.

use crate::graph::main::nodes::closure::closure;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::{PyFrozenSet, PySetMethods};

/// The upstream key closure of one compiled graph key.
pub fn expand_project_upstream_keys<'py>(
    key: Bound<'py, PyAny>,
    upstream_deps: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyFrozenSet>> {
    let visited = closure(vec![key], upstream_deps, (false, None))?;
    PyFrozenSet::new(upstream_deps.py(), visited.iter())
}
