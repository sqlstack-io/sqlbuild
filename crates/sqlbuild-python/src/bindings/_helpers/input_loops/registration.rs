use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_compile::input_loops::constants::RESOURCE_ATTACHMENT;
use sqlbuild_python_compile::input_loops::main::register_attachment::register_attachment;
use sqlbuild_python_compile::input_loops::main::register_expanded_offsets::register_expanded_offsets;
use sqlbuild_python_compile::input_loops::main::register_model_loop::register_model_loop;
use sqlbuild_python_project::compiled_project::main::install::install;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    install(RESOURCE_ATTACHMENT);
    register_model_loop(module)?;
    register_attachment(module)?;
    register_expanded_offsets(module)
}
