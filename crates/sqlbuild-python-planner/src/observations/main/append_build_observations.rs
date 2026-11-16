//! Record successful incremental models' cursor `MAX` and the source columns planning described; failures only warn.

use crate::observations::_helpers::publication::CursorRead;
use crate::observations::_helpers::publication::cursor_reads;
use crate::observations::_helpers::publication::observe;
use crate::observations::_helpers::publication::source_column_rows;
use crate::observations::main::take_source_capture::take_source_capture;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_front::native_host::models::ObservationAppend;
use sqlbuild_python_front::native_host::models::{ConsumptionConnection, Location};
use std::collections::BTreeMap;

/// Record successful incremental models' cursor `MAX` and the source columns planning described; failures only warn.
pub fn append_build_observations(
    (project, successful_models): (&Bound<'_, PyAny>, &[String]),
    (adapter, connection_config): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    run_id: &str,
) -> Vec<String> {
    let described: BTreeMap<String, String> = take_source_capture().into_iter().collect();
    let recorded: PyResult<Vec<String>> = (|| {
        let reads: Vec<CursorRead> = cursor_reads(project, adapter, successful_models)?;
        let sources: BTreeMap<Location, Vec<ObservationAppend>> =
            source_column_rows(project, &described, run_id)?;
        if reads.is_empty() && sources.is_empty() {
            return Ok(Vec::new());
        }
        let py: Python<'_> = adapter.py();
        let connection = ConsumptionConnection::new(adapter, connection_config);
        let observed: PyResult<Vec<String>> = connection
            .connected(py)
            .and_then(|handle| observe(&reads, sources, (adapter, &handle), run_id));
        let closed: PyResult<()> = connection.close(py);
        let warnings: Vec<String> = observed?;
        closed?;
        Ok(warnings)
    })();
    recorded.unwrap_or_else(|error| {
        vec![format!(
            "could not record observations: {error}; plan previews show them as unknown"
        )]
    })
}
