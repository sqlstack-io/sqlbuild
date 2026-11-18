//! Public traits of native host.

use crate::native_host::models::Location;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
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
