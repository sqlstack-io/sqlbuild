//! Register planner identity, pruning and graph-order bindings.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::planner_identity::{identities, metadata, staleness};
use sqlbuild_python_project::planner_identity::main::register_graph::register_graph;
use sqlbuild_python_project::planner_identity::main::register_hashing::register_hashing;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_graph(module)?;
    register_hashing(module)?;
    identities::register(module)?;
    metadata::register(module)?;
    staleness::register(module)
}
