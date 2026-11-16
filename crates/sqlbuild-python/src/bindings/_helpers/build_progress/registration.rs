//! Register the native build, audit and test progress reporters, the build footer and the transient status.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::build_progress::{
    audit_progress, build_callbacks, footer, nested_progress,
};
use sqlbuild_python_base::build_progress::main::terminal::register_terminal::register_terminal;
use sqlbuild_python_base::build_progress::main::transient_lines::register_transient_lines::register_transient_lines;
use sqlbuild_python_base::build_progress::main::transient_status::register_transient_status::register_transient_status;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    audit_progress::register(module)?;
    build_callbacks::register(module)?;
    footer::register(module)?;
    nested_progress::register(module)?;
    register_terminal(module)?;
    register_transient_lines(module)?;
    register_transient_status(module)
}
