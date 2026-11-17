//! Lifecycle regions of a run's selected Python nodes, classified natively.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::python_plan::models::LifecycleNode;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_analysis::assembly::project::types::ObjectKey;

pub(crate) fn lifecycle_node(node: &Bound<'_, PyAny>) -> PyResult<LifecycleNode> {
    let mut sql_deps: Vec<ObjectKey> = Vec::new();
    for reference in native_field(node, "sql_deps")?.try_iter()? {
        let reference: Bound<'_, PyAny> = reference?;
        sql_deps.push((
            native_field(&reference, "kind")?.extract()?,
            native_field(&reference, "name")?.extract()?,
        ));
    }
    Ok(LifecycleNode {
        kind: native_field(node, "kind")?.extract()?,
        sql_deps,
    })
}
