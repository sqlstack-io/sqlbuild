//! Public records of freshness.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use crate::planner_warehouse::models::PlannerReads;
use sqlbuild_python_base::native_host::models::Dialect;

/// Previous state records by identity; absent means no comparison.
#[derive(Debug)]
pub struct PreviousState<'py> {
    pub by_identity: Option<Bound<'py, PyAny>>,
}

<<<<<<< ours
/// The SQL dialect, warehouse reads and observation time every source is observed with.
pub struct Observer<'py> {
    pub dialect: Bound<'py, Dialect>,
    pub reads: Bound<'py, PlannerReads>,
=======
/// The adapter, connection and observation time every source is observed with.
#[derive(Debug)]
pub struct Observer<'py> {
    pub adapter: Bound<'py, PyAny>,
    pub connection: Bound<'py, PyAny>,
>>>>>>> theirs
    pub observed_at: Bound<'py, PyAny>,
}

/// The callbacks and location one latest-state read uses.
#[derive(Debug)]
pub struct StateRead<'py> {
    pub connection: Bound<'py, PyAny>,
    pub execute: Bound<'py, PyAny>,
    pub render_qualified_name: Bound<'py, PyAny>,
    pub render_read_latest_sql: Bound<'py, PyAny>,
}
