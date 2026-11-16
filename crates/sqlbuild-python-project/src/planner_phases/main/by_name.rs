//! The project nodes of `field` by name.

use crate::compiled_views::main::project::collection::collection;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

/// The project nodes of `field` by name.
pub fn by_name<'py>(
    project: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Bound<'py, pyo3::types::PyDict>> {
    let nodes = pyo3::types::PyDict::new(project.py());
    for node in collection(project, field)?.try_iter()? {
        let node = node?;
        pyo3::types::PyDictMethods::set_item(
            &nodes,
            crate::resource_views::main::resource_reads::name::name(&node)?,
            node,
        )?;
    }
    Ok(nodes)
}
