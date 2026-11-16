//! Lifecycle regions of a run's selected Python nodes, classified natively.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::python_plan::models::LifecycleNode;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const EXTERNAL_CONNECTION_MODE: &str = "external";

pub(crate) fn lifecycle_node(node: &Bound<'_, PyAny>) -> PyResult<LifecycleNode> {
    let loader: Bound<'_, PyAny> = native_field(node, "loader")?;
    let external_loader: bool = !loader.is_none()
        && native_field(&loader, "connection_mode")?.extract::<String>()?
            == EXTERNAL_CONNECTION_MODE;
    Ok(LifecycleNode {
        kind: native_field(node, "kind")?.extract()?,
        external_loader,
    })
}
