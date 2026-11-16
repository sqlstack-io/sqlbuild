//! The discovered Python nodes by kind, name and path only, as the resource-name check reads them.

use crate::discovery::_helpers::project::project_facts;
use crate::discovery::models::NodeRow;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The discovered Python nodes by kind, name and path only, as the resource-name check reads them.
#[inline]
pub fn named_node_rows(inputs: &Bound<'_, PyAny>) -> PyResult<Vec<NodeRow>> {
    project_facts::named_node_rows(inputs)
}
