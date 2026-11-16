//! Keys.

use crate::graph::main::nodes::native_key::native_key;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_analysis::assembly::project::types::ObjectKey;

/// Keys.
pub fn keys(values: &Bound<'_, PyAny>) -> PyResult<Vec<ObjectKey>> {
    values
        .try_iter()?
        .map(|value| native_key(&value?))
        .collect()
}
