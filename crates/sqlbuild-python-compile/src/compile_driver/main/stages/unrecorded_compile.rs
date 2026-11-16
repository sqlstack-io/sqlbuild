//! The resolved request, output recorder and phases of a compile neither stored nor printed.

use crate::compile_command::models::RecordedCompileOutput;
use crate::compile_driver::_helpers::stages::command::collecting_later;
use crate::compile_driver::_helpers::stages::command::compile_phases;
use crate::compile_driver::main::stages::resolved_request::resolved_request;
use crate::compile_driver::types::PhasesOutcome;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::boundary::models::Status;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The resolved request, output recorder and phases of a compile neither stored nor printed.
pub fn unrecorded_compile<'py>(
    request: &Bound<'py, PyAny>,
    attempt: &Bound<'py, PyAny>,
) -> PyResult<(
    Bound<'py, PyAny>,
    Bound<'py, PyAny>,
    PyResult<PhasesOutcome<'py>>,
)> {
    let py: Python<'py> = request.py();
    let total_start: f64 = native_field(attempt, "started")?.extract()?;
    let effective_request: Bound<'py, PyAny> = resolved_request(request)?;
    let output: Bound<'py, PyAny> = Bound::new(py, RecordedCompileOutput::new())?.into_any();
    let status: Status<'py> = Status(py.None().into_bound(py));
    let phases: PyResult<PhasesOutcome<'py>> = collecting_later(py, || {
        compile_phases(
            (&effective_request, attempt),
            (&output, &status),
            total_start,
        )
    })?;
    Ok((effective_request, output, phases))
}
