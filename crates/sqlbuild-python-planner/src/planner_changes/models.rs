//! Public records of planner changes.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// Inputs shared by every model of one detection pass.
#[derive(Debug)]
pub struct ModelPass<'py> {
    pub snapshot: Bound<'py, PyAny>,
    pub query_change_tracking: bool,
    pub full_refresh: bool,
    pub function_local_hashes: Bound<'py, PyAny>,
    pub hook_version_hashes: Bound<'py, PyAny>,
}
