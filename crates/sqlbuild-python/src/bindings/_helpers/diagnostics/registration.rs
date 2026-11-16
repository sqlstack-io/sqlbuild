//! Register diagnostics console formatting, redaction and process trackers.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::diagnostics::main::register_console_records::register_console_records;
use sqlbuild_python_base::diagnostics::main::register_log_records::register_log_records;
use sqlbuild_python_base::diagnostics::main::register_redaction::register_redaction;
use sqlbuild_python_base::diagnostics::main::register_trackers::register_trackers;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_console_records(module)?;
    register_log_records(module)?;
    register_redaction(module)?;
    register_trackers(module)
}
