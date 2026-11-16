//! Append observed source freshness only after the affected selected models succeed.

use crate::freshness::_helpers::publication::RecordsByLocation;
use crate::freshness::_helpers::publication::eligible_records;
use crate::freshness::_helpers::publication::write_records;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::within::within;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;

/// Append observed source freshness only after the affected selected models succeed.
pub fn append_freshness_records<'py>(
    (plan, successful_models): (&Bound<'py, PyAny>, &[String]),
    adapter: &Bound<'py, PyAny>,
    connection_config: &Bound<'py, PyAny>,
    run_id: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let py: Python<'py> = plan.py();
    let grouped: RecordsByLocation<'py> = eligible_records(plan, successful_models, run_id)?;
    let record_count: usize = grouped.iter().map(|(_, records)| records.len()).sum();
    if record_count == 0 {
        return Ok(());
    }
    let run_scope: Bound<'py, PyAny> = host_entry(py, ExecutorHost::RunScope)?.call1((run_id,))?;
    within(&run_scope, |_| {
        let metadata: Bound<'py, PyDict> = PyDict::new(py);
        metadata.set_item("item_count", record_count)?;
        let attributes: Bound<'py, PyAny> = call_with(
            &host_entry(py, ExecutorHost::OperationAttributes)?,
            &[
                ("phase", PyString::new(py, "publish").into_any()),
                ("target_kind", PyString::new(py, "state_batch").into_any()),
            ],
        )?;
        let lifecycle: Bound<'py, PyAny> = call_with(
            &host_entry(py, ExecutorHost::OperationLifecycle)?,
            &[
                ("operation_kind", PyString::new(py, "freshness").into_any()),
                (
                    "operation_name",
                    PyString::new(py, "source_freshness_publication").into_any(),
                ),
                ("metadata", metadata.into_any()),
                ("attributes", attributes),
            ],
        )?;
        within(&lifecycle, |_| {
            let connection: Bound<'py, PyAny> =
                adapter.call_method1("connect", (connection_config,))?;
            let written: PyResult<()> = write_records(adapter, &connection, &grouped);
            let closed: PyResult<Bound<'py, PyAny>> = adapter.call_method1("close", (&connection,));
            written?;
            closed?;
            Ok(())
        })
    })
}
