//! The process clock compile CPU timings read: Python's `time.process_time_ns`.

use pyo3::PyErr;
use pyo3::prelude::{PyAnyMethods, PyResult, Python};
use sqlbuild_host::compile_context::types::ProcessClock;

use sqlbuild_python_base::boundary::main::imported::imported;

/// The process CPU clock of one interpreter, including its worker threads.
#[derive(Clone, Copy)]
pub(crate) struct PythonProcessClock<'py>(pub(crate) Python<'py>);

impl ProcessClock for PythonProcessClock<'_> {
    type Error = PyErr;

    fn process_time_ns(&self) -> PyResult<u64> {
        imported(self.0, "time", "process_time_ns")?
            .call0()?
            .extract()
    }
}
