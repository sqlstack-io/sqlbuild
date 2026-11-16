//! Register the manifest builder.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_compile::manifest::main::register_builder::register_builder;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_builder(module)
}
