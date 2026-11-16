//! Register the generated relation name fitting functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_project::relation_naming::main::register_fitting::register_fitting;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_fitting(module)
}
