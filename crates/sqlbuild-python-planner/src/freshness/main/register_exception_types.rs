//! Register the exception types functions and classes.

use crate::freshness::_helpers::exception_types::SourceFreshnessInputError;
use crate::freshness::_helpers::exception_types::SourceFreshnessObservationError;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the exception types functions and classes.
pub fn register_exception_types(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py = module.py();
    module.add(
        "SourceFreshnessInputError",
        py.get_type::<SourceFreshnessInputError>(),
    )?;
    module.add(
        "SourceFreshnessObservationError",
        py.get_type::<SourceFreshnessObservationError>(),
    )
}
