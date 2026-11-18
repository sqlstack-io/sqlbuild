//! Metadata estimates never execute exact counts; one adapter read serves each page of names.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    StatisticsRead, WarehouseHistoryPage, WarehouseHistoryRecord, WarehouseStatisticsRecord,
};

use crate::boundary::main::call_with::call_with;
use crate::records::main::fields::field::field;

pub(crate) fn read_statistics(
    py: Python<'_>,
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    request: &StatisticsRead,
) -> PyResult<WarehouseHistoryPage> {
    if request.names.len() > MAX_HISTORY_RECORDS
        || request.names.iter().any(|name| name.trim().is_empty())
    {
        return Err(PyValueError::new_err(
            "statistics sampling needs at most 100 exact relation names",
        ));
    }
    let estimates = call_with(
        &adapter.getattr("estimate_relation_row_counts")?,
        &[
            ("connection", connection.clone()),
            (
                "database",
                request.location.database.clone().into_bound_py_any(py)?,
            ),
            (
                "schema",
                request.location.schema.clone().into_bound_py_any(py)?,
            ),
            ("names", PyTuple::new(py, &request.names)?.into_any()),
        ],
    )?;
    let observed_at = chrono::Utc::now();
    let mut records: Vec<WarehouseHistoryRecord> = Vec::new();
    for (name, estimate) in request.names.iter().zip(estimates.try_iter()?) {
        let estimate = estimate?;
        records.push(WarehouseHistoryRecord::Statistics(
            WarehouseStatisticsRecord {
                state_database: request.location.database.clone(),
                state_schema: request.location.schema.clone(),
                name: name.clone(),
                observed_at,
                row_count: field(&estimate, "row_count")?.extract()?,
                detail: field(&estimate, "detail")?.extract()?,
                byte_count: field(&estimate, "byte_count")?.extract()?,
                byte_count_basis: field(&estimate, "byte_count_basis")?.extract()?,
            },
        ));
    }
    if records.len() != request.names.len() {
        return Err(PyValueError::new_err(
            "the adapter estimated a different number of relations than were named",
        ));
    }
    Ok(WarehouseHistoryPage {
        records,
        next_after: request.next_after.map(|id| vec![Some(id.to_string())]),
        change_token: None,
        unchanged: false,
    })
}
