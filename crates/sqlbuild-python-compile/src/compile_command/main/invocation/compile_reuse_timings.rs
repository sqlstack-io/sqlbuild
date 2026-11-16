//! The hit, miss and bypass counters plus the reuse check time, keyed as `compile_timings` is.

use crate::compile_command::_helpers::invocation::reuse_recording::BYPASS_OUTCOME;
use crate::compile_command::_helpers::invocation::reuse_recording::HIT_OUTCOME;
use crate::compile_command::_helpers::invocation::reuse_recording::MISS_OUTCOME;
use crate::compile_command::constants::REUSE_BYPASS_TIMING;
use crate::compile_command::constants::REUSE_CHECK_TIMING;
use crate::compile_command::constants::REUSE_HIT_TIMING;
use crate::compile_command::constants::REUSE_MISS_TIMING;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The hit, miss and bypass counters plus the reuse check time, keyed as `compile_timings` is.
pub fn compile_reuse_timings<'py>(attempt: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = attempt.py();
    let outcome: String = native_field(attempt, "outcome")?.str()?.to_string();
    let timings: Bound<'py, PyDict> = PyDict::new(py);
    for (name, counted) in [
        (REUSE_HIT_TIMING, HIT_OUTCOME),
        (REUSE_MISS_TIMING, MISS_OUTCOME),
        (REUSE_BYPASS_TIMING, BYPASS_OUTCOME),
    ] {
        timings.set_item(name, i64::from(outcome == counted))?;
    }
    timings.set_item(REUSE_CHECK_TIMING, native_field(attempt, "check_ms")?)?;
    Ok(timings)
}
