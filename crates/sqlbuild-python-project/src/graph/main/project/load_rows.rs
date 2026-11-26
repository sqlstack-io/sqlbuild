//! The graph rows of a project's loads: each a node with no dependencies, tags or folder.

use crate::graph::constants::LOAD_RESOURCE;
use crate::graph::models::ResourceRow;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// One graph row per `CompiledLoad` in `loads`, in project order.
pub fn load_rows(loads: &Bound<'_, PyAny>) -> PyResult<Vec<ResourceRow>> {
    loads
        .try_iter()?
        .map(|load| {
            Ok(ResourceRow {
                key: (
                    LOAD_RESOURCE.to_owned(),
                    native_field(&load?, "name")?.extract()?,
                ),
                deps: Vec::new(),
                tags: Vec::new(),
                folder: None,
            })
        })
        .collect()
}
