//! Register native command-output capture.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_output::output_capture::main::register_context::register_context;
use sqlbuild_python_output::output_capture::main::register_dispatcher::register_dispatcher;
use sqlbuild_python_output::output_capture::main::register_records::register_records;
use sqlbuild_python_output::output_capture::main::register_text::register_text;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_context(module)?;
    register_dispatcher(module)?;
    register_records(module)?;
    register_text(module)
}
