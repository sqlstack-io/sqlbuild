//! Register the native build, audit and test progress reporters, the build footer and the transient status.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::build_progress::main::terminal::register_terminal::register_terminal;
use sqlbuild_python_base::build_progress::main::transient_lines::register_transient_lines::register_transient_lines;
use sqlbuild_python_base::build_progress::main::transient_status::register_transient_status::register_transient_status;
use sqlbuild_python_output::build_progress::main::register_audit_progress::register_audit_progress;
use sqlbuild_python_output::build_progress::main::register_build_callbacks::register_build_callbacks;
use sqlbuild_python_output::build_progress::main::register_footer::register_footer;
use sqlbuild_python_output::build_progress::main::register_nested_progress::register_nested_progress;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_audit_progress(module)?;
    register_build_callbacks(module)?;
    register_footer(module)?;
    register_nested_progress(module)?;
    register_terminal(module)?;
    register_transient_lines(module)?;
    register_transient_status(module)
}
