//! Register the native relation shape, coverage and declaration functions.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_project::semantic_shapes::main::register_coverage::register_coverage;
use sqlbuild_python_project::semantic_shapes::main::register_shapes::register_shapes;
use sqlbuild_python_project::semantic_shapes::main::register_source_bindings::register_source_bindings;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_shapes(module)?;
    register_coverage(module)?;
    register_source_bindings(module)
}
