//! Register the native compile output functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_base::compile_outputs::main::register_json_report::register_json_report;
use sqlbuild_python_compile::compile_outputs::main::register_reuse::register_reuse;
use sqlbuild_python_compile::compile_outputs::main::register_runtime_target::register_runtime_target;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_json_report(module)?;
    register_reuse(module)?;
    register_runtime_target(module)
}
