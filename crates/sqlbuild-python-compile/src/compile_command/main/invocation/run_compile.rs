//! Run one compile command request, replaying the previous compile when nothing changed.

use crate::compile_command::_helpers::invocation::reuse_attempt::HIT;
use crate::compile_command::_helpers::invocation::reuse_attempt::MISS;
use crate::compile_command::_helpers::invocation::reuse_attempt::attempt_compile_reuse;
use crate::compile_command::_helpers::invocation::reuse_attempt::compiled_with_delta;
use crate::compile_command::_helpers::invocation::reuse_attempt::outcome;
use crate::compile_command::_helpers::invocation::reuse_attempt::reuse_request;
use crate::compile_delta::main::driver::delta_enabled::delta_enabled;
use crate::compile_delta::main::verify::disarm::disarm;
use crate::compile_driver::main::stages::run_compile_command::run_compile_command;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::compile_context::main::delta::reset::reset;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Run one compile command request, replaying the previous compile when nothing changed.
#[pyfunction]
pub fn run_compile<'py>(request: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.py();
    let attempt: Bound<'py, PyAny> = attempt_compile_reuse(&reuse_request(request)?)?;
    let exit_code: Bound<'py, PyAny> = native_field(&attempt, "exit_code")?;
    if native_field(&attempt, "outcome")?.eq(outcome(py, HIT)?)? && !exit_code.is_none() {
        return Ok(exit_code);
    }
    let missed: bool = native_field(&attempt, "outcome")?.eq(outcome(py, MISS)?)?;
    let code: PyResult<i64> = if missed && delta_enabled() {
        compiled_with_delta(request, &attempt)
    } else {
        run_compile_command(py, request.clone(), Some(attempt))
    };
    reset();
    disarm();
    code.and_then(|value| pyo3::IntoPyObjectExt::into_bound_py_any(value, py))
}
