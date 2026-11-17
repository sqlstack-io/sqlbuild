//! Register lifecycle event catalogue queries, constants and envelope validation.

use pyo3::prelude::{Bound, PyModule, PyResult};

use sqlbuild_python_planner::lifecycle_events::main::register_catalog::register_catalog;
use sqlbuild_python_planner::lifecycle_events::main::register_envelopes::register_envelopes;
use sqlbuild_python_planner::lifecycle_events::main::register_validation::register_validation;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_catalog(module)?;
    register_envelopes(module)?;
    register_validation(module)
}
