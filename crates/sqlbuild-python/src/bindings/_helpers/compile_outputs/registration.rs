//! Register the native compile output functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::compile_outputs::{reuse, runtime_target};
use sqlbuild_python_base::compile_outputs::main::register_json_report::register_json_report;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_json_report(module)?;
    reuse::register(module)?;
    runtime_target::register(module)
}
