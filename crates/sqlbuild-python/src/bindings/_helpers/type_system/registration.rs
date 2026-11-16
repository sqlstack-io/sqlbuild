//! Register the native type system functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::type_system::main::register_normalization::register_normalization;
use sqlbuild_python_project::type_system::main::register_nullability::register_nullability;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_normalization(module)?;
    register_nullability(module)?;
    Ok(())
}
