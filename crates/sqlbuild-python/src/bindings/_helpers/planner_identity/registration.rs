//! Register planner identity, pruning and graph-order bindings.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_planner::planner_identity::main::register_identities::register_identities;
use sqlbuild_python_planner::planner_identity::main::register_metadata::register_metadata;
use sqlbuild_python_planner::planner_identity::main::register_staleness::register_staleness;
use sqlbuild_python_project::planner_identity::main::register_graph::register_graph;
use sqlbuild_python_project::planner_identity::main::register_hashing::register_hashing;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_graph(module)?;
    register_hashing(module)?;
    register_identities(module)?;
    register_metadata(module)?;
    register_staleness(module)
}
