//! Register the native microbatch history projections and statements.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_planner::microbatches::main::register_projection::register_projection;
use sqlbuild_python_planner::microbatches::main::register_statements::register_statements;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_projection(module)?;
    register_statements(module)
}
