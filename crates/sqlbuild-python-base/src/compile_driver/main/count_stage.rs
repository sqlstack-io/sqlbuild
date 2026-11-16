//! Add `count` runs of `stage`.

use crate::compile_driver::_helpers::stage_work;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// Add `count` runs of `stage`.
#[inline]
pub fn count_stage(py: Python<'_>, stage: &'static str, count: u64) -> PyResult<()> {
    stage_work::count_stage(py, stage, count)
}
