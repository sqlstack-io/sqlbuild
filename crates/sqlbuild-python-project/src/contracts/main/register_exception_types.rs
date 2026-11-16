//! Register the exception types functions and classes.

use crate::contracts::_helpers::exception_types::SqlValueValidationError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the exception types functions and classes.
pub fn register_exception_types(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add(
        "SqlValueValidationError",
        module.py().get_type::<SqlValueValidationError>(),
    )
}
