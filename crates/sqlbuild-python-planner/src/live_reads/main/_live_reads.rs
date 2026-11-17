//! Live warehouse reads for planner functions called from Python.

use crate::live_reads::models::LiveReads;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use std::sync::Arc;

/// Live warehouse reads for planner functions called from Python, optionally observing statements.
#[pyfunction]
#[pyo3(signature = (*, adapter, connection, connection_config = None, execute = None))]
pub(in crate::live_reads) fn live_reads<'py>(
    adapter: &Bound<'py, PyAny>,
    connection: &Bound<'py, PyAny>,
    connection_config: Option<Bound<'py, PyAny>>,
    execute: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PlannerReads>> {
    let mut reads = LiveReads::new(adapter, connection, connection_config.as_ref());
    reads.execute = execute
        .filter(|execute| !execute.is_none())
        .map(Bound::unbind);
    Bound::new(adapter.py(), PlannerReads::new(Arc::new(reads)))
}
