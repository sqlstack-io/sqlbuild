//! Run a capture hook; a failure only rules the compile out as a delta base.

use crate::compile_context::_helpers::delta;
use pyo3::prelude::PyResult;

/// Run a capture hook; a failure only rules the compile out as a delta base.
#[inline]
pub fn guarded(hook: impl FnOnce() -> PyResult<()>) -> PyResult<()> {
    delta::guarded(hook)
}
