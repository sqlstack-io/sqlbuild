//! A `CostRunRecord` as a plain run.

use crate::resource_costs::_helpers::cost_run_records::integer;
use crate::resource_costs::_helpers::cost_run_records::optional_text;
use crate::resource_costs::_helpers::cost_run_records::text;
use crate::resource_costs::_helpers::cost_run_records::timestamp;
use crate::resource_costs::main::summary::summary;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::cost_report::models::CostRun;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// A `CostRunRecord` as a plain run.
pub fn cost_run(record: &Bound<'_, PyAny>) -> PyResult<CostRun> {
    Ok(CostRun {
        run_id: text(record, "run_id")?,
        adapter_name: text(record, "adapter_name")?,
        target_name: optional_text(record, "target_name")?,
        build_status: text(record, "build_status")?,
        started_at: timestamp(record, "started_at")?,
        completed_at: timestamp(record, "completed_at")?,
        cost: summary(&native_field(record, "cost")?)?,
        version: integer(record, "version")?,
        had_executable_work: native_field(record, "had_executable_work")?.extract()?,
    })
}
