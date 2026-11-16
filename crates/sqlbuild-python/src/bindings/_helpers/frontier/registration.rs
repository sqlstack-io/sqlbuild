//! Register the compiler frontier and its stage captures.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_compile::frontier::main::register_cache_directory::register_cache_directory;
use sqlbuild_python_compile::frontier::main::register_compile_frontier::register_compile_frontier;
use sqlbuild_python_compile::frontier::main::register_stage_capture::register_stage_capture;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_cache_directory(module)?;
    register_compile_frontier(module)?;
    register_stage_capture(module)
}
