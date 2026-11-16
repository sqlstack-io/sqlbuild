//! Public records of freshness.

use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;

/// Previous state records by identity; absent means no comparison.
#[derive(Debug)]
pub struct PreviousState<'py> {
    pub by_identity: Option<Bound<'py, PyAny>>,
}

/// The adapter, connection and observation time every source is observed with.
#[derive(Debug)]
pub struct Observer<'py> {
    pub adapter: Bound<'py, PyAny>,
    pub connection: Bound<'py, PyAny>,
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
