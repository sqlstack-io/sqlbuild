//! Register the native type system functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::type_system::nullability;
use sqlbuild_python_base::type_system::main::register_normalization::register_normalization;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_normalization(module)?;
    nullability::register(module)?;
    Ok(())
}
