//! Whether an adapter implements the cost telemetry methods.

use crate::cost_context::constants::COST_AWARE_METHODS;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Whether `adapter` implements the cost telemetry methods.
pub fn cost_aware(adapter: &Bound<'_, PyAny>) -> PyResult<bool> {
    for name in COST_AWARE_METHODS {
        if !adapter.hasattr(name)? || native_field(adapter, name)?.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}
