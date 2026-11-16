//! Register `compile_stage_work_info` on the native module.

use crate::compile_driver::_helpers::stage_work;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// Register `compile_stage_work_info` on the native module.
#[inline]
pub fn register_stage_work(module: &Bound<'_, PyModule>) -> PyResult<()> {
    stage_work::register(module)
}
