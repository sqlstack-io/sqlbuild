//! Lifecycle regions of a run's selected Python nodes, classified natively.

use crate::graph::_helpers::python_nodes::python_selection::sql_deps;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::python_plan::models::LifecycleNode;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

pub(crate) fn lifecycle_node(node: &Bound<'_, PyAny>) -> PyResult<LifecycleNode> {
    Ok(LifecycleNode {
        kind: native_field(node, "kind")?.extract()?,
        sql_deps: sql_deps(node)?,
    })
}
