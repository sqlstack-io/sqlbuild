//! Upstream loader dependency names.

use crate::planner_scope::_helpers::values::{dependency_names, loader_graph, loader_names};
use pyo3::prelude::{Bound, PyAny, PyResult, pyfunction};
use pyo3::types::PyTuple;

#[pyfunction]
#[pyo3(signature = (*, loader_function, loader_functions))]
pub fn upstream_loader_dependency_names<'py>(
    loader_function: &Bound<'py, PyAny>,
    loader_functions: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let graph = loader_graph(loader_functions)?;
    let names = loader_names(loader_functions)?;
    let deps = dependency_names(loader_function, &names)?;
    PyTuple::new(loader_function.py(), graph.upstream(&deps))
}
