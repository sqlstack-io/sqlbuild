//! Records of several plain runs, as a tuple.

use crate::resource_costs::main::record_object::record_object;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_core::cost_report::models::CostRun;

/// Records of several plain runs, as a tuple.
pub fn record_tuple<'py>(py: Python<'py>, runs: &[CostRun]) -> PyResult<Bound<'py, PyTuple>> {
    let mut records: Vec<Bound<'py, PyAny>> = Vec::with_capacity(runs.len());
    for run in runs {
        records.push(record_object(py, run)?);
    }
    PyTuple::new(py, records)
}
