//! Register the live warehouse reads.

use crate::live_reads::main::_live_reads::live_reads;
use crate::planner_warehouse::models::PlannerReads;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

pub fn register_live_reads(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PlannerReads>()?;
    module.add_function(wrap_pyfunction!(live_reads, module)?)?;
    Ok(())
}
