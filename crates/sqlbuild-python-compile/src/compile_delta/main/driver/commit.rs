//! Store and print a finished delta compile; returns its exit code.

use crate::compile_command::main::invocation::print_to::print_to;
use crate::compile_command::models::RecordedCompileOutput;
use crate::compile_delta::_helpers::driver::STDERR;
use crate::compile_delta::models::Finished;
use crate::compile_driver::main::stages::resolved_request::resolved_request;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

/// Store and print a finished delta compile; returns its exit code.
pub fn commit(
    finished: Finished<'_>,
    (request, attempt): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<i64> {
    let py: Python<'_> = attempt.py();
    let request: Bound<'_, PyAny> = resolved_request(request)?;
    let Finished {
        stdout,
        stderr_lines,
        phases,
        image,
        merge,
        ..
    } = finished;
    let report: &str = stdout.strip_suffix('\n').unwrap_or(&stdout);
    let output: Bound<'_, PyAny> = Bound::new(
        py,
        RecordedCompileOutput::recorded(py, &stderr_lines, &format!("{report}\n")),
    )?
    .into_any();
    for line in &stderr_lines {
        print_to(py, STDERR, line.as_str())?;
    }
    sqlbuild_python_base::compile_context::main::delta::set_pending_image::set_pending_image(image);
    sqlbuild_python_base::compile_context::main::delta::set_record_merge::set_record_merge(Some(
        merge,
    ));
    crate::compile_driver::main::stages::store_and_print::store_and_print(
        (&request, attempt),
        &output,
        phases,
    )
}
