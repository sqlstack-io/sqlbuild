//! Edges.

use crate::graph::main::nodes::native_key::native_key;
use crate::planner_scope::main::keys::keys;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;

/// Edges.
pub fn edges(values: &Bound<'_, PyAny>) -> PyResult<Vec<(ObjectKey, Vec<ObjectKey>)>> {
    mapping_items(values)?
        .try_iter()?
        .map(|item| {
            let (key, deps): (Bound<'_, PyAny>, Bound<'_, PyAny>) = item?.extract()?;
            Ok((native_key(&key)?, keys(&deps)?))
        })
        .collect()
}
