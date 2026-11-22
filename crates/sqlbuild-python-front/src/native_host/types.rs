//! Public traits of native host.

use crate::native_host::models::Location;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::prelude::{Bound, PyAny};
use sqlbuild_state::consumption::models::ConsumptionRow;

/// Reading recorded consumption: from the warehouse table, or from a synced state.
pub trait ConsumptionReads {
    /// The newest executions per relation recorded in `location`.
    fn read_recent(&self, py: Python<'_>, location: &Location) -> PyResult<Vec<ConsumptionRow>>;
    /// The named executions recorded in `location`.
    fn read_executions(
        &self,
        py: Python<'_>,
        location: &Location,
        execution_ids: &[&str],
    ) -> PyResult<Vec<ConsumptionRow>>;
}

/// One state location's database and schema, and the pause table token its last quiet pass left.
pub type ResumeLocation = ((Option<String>, String), Option<String>);

/// Each resumed hold's run ID and dotted relation, each location's new quiet token, and failures.
pub type ResumedHolds = (Vec<(String, String)>, Vec<ResumeLocation>, Vec<String>);

/// A project's adapter and the connection configuration of its selected target.
pub type ResumeConnection<'a, 'py> = (&'a Bound<'py, PyAny>, &'a Bound<'py, PyAny>);
