//! Run one full compile, recording it for reuse; returns the exit code.

use crate::compile_command::models::RecordedCompileOutput;
use crate::compile_driver::_helpers::stages::command::bypass_attempt;
use crate::compile_driver::_helpers::stages::command::collecting_later;
use crate::compile_driver::_helpers::stages::command::compile_status;
use crate::compile_driver::_helpers::stages::command::recorded_compile;
use crate::compile_driver::main::stages::monotonic::monotonic;
use crate::compile_driver::main::stages::resolved_request::resolved_request;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::boundary::models::Status;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// Run one full compile, recording it for reuse; returns the exit code.
#[pyfunction]
#[pyo3(signature = (request, reuse_attempt=None))]
pub fn run_compile_command<'py>(
    py: Python<'py>,
    request: Bound<'py, PyAny>,
    reuse_attempt: Option<Bound<'py, PyAny>>,
) -> PyResult<i64> {
    let total_start: f64 = match &reuse_attempt {
        Some(attempt) => native_field(attempt, "started")?.extract()?,
        None => monotonic(py)?,
    };
    let effective_request: Bound<'py, PyAny> = resolved_request(&request)?;
    let attempt: Bound<'py, PyAny> = match reuse_attempt {
        Some(attempt) => attempt,
        None => bypass_attempt(py, &effective_request, total_start)?,
    };
    let output: Bound<'py, PyAny> = Bound::new(py, RecordedCompileOutput::new())?.into_any();
    let status: Status<'py> = Status(compile_status(&request)?);
    collecting_later(py, || {
        recorded_compile(
            (&effective_request, &attempt),
            (&output, &status),
            total_start,
        )
    })?
}
