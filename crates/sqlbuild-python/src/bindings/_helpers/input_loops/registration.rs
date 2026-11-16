use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::input_loops::resource_attachment::RESOURCE_ATTACHMENT;
use sqlbuild_python_project::compiled_project::main::install::install;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    install(RESOURCE_ATTACHMENT);
    crate::bindings::_helpers::input_loops::model_loop::register(module)?;
    crate::bindings::_helpers::input_loops::attachment::register(module)?;
    crate::bindings::_helpers::input_loops::expanded_offsets::register(module)
}
