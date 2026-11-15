//! Register oracles.

use crate::boundary::_helpers::oracles;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// register_oracles.
#[inline]
pub fn register_oracles(module: &Bound<'_, PyModule>) -> PyResult<()> {
    oracles::register(module)
}
