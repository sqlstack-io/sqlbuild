//! Register `declaration_site` on the native module.

use crate::orchestration_reads::_helpers::orchestration_declarations;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register `declaration_site` on the native module.
#[inline]
pub fn register_orchestration_declarations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    orchestration_declarations::register(module)
}
