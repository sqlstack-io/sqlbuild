//! Register planner identity, pruning and graph-order bindings.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::planner_identity::graph;
use crate::bindings::_helpers::planner_identity::hashing;
use crate::bindings::_helpers::planner_identity::identities;
use crate::bindings::_helpers::planner_identity::metadata;
use crate::bindings::_helpers::planner_identity::staleness;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    graph::register(module)?;
    hashing::register(module)?;
    identities::register(module)?;
    metadata::register(module)?;
    staleness::register(module)
}
