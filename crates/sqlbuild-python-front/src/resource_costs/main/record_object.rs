//! A plain run as the `CostRunRecord` it was read from.

use crate::resource_costs::_helpers::cost_run_records::Builders;
use crate::resource_costs::_helpers::cost_run_records::summary_object;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_core::cost_report::models::CostRun;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::types::RecordId;

/// A plain run as the `CostRunRecord` it was read from.
pub fn record_object<'py>(py: Python<'py>, run: &CostRun) -> PyResult<Bound<'py, PyAny>> {
    let builders: Builders<'py> = Builders::new(py)?;
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("version", run.version)?;
    fields.set_item("run_id", &run.run_id)?;
    fields.set_item("adapter_name", &run.adapter_name)?;
    fields.set_item("target_name", &run.target_name)?;
    fields.set_item("build_status", &run.build_status)?;
    fields.set_item(
        "started_at",
        builders.timestamp.call1((run.started_at.as_str(),))?,
    )?;
    fields.set_item(
        "completed_at",
        builders.timestamp.call1((run.completed_at.as_str(),))?,
    )?;
    fields.set_item("cost", summary_object(&builders, &run.cost)?)?;
    fields.set_item("had_executable_work", run.had_executable_work)?;
    new_record(py, RecordId::CostRunRecord, (), Some(&fields))
}
