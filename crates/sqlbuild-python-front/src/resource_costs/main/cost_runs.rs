//! Every `CostRunRecord` of an iterable as a plain run.

use crate::resource_costs::main::cost_run::cost_run;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::cost_report::models::CostRun;

/// Every record of an iterable as a plain run.
pub fn cost_runs(records: &Bound<'_, PyAny>) -> PyResult<Vec<CostRun>> {
    let mut runs: Vec<CostRun> = Vec::new();
    for record in records.try_iter()? {
        runs.push(cost_run(&record?)?);
    }
    Ok(runs)
}
