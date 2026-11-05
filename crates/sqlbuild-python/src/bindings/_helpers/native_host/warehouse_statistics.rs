//! Metadata estimates never execute exact counts or imply source-data currency.

use pyo3::IntoPyObjectExt;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_orchestration::warehouse_metadata::constants::MAX_HISTORY_RECORDS;
use sqlbuild_orchestration::warehouse_metadata::models::{
    StatisticsRead, WarehouseHistoryPage, WarehouseHistoryRecord, WarehouseStatisticsRecord,
};

use crate::bindings::_helpers::boundary::python_calls::call_with;
use crate::bindings::_helpers::records::fields::field;

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
    let mut records: Vec<WarehouseHistoryRecord> = Vec::new();
    for name in &request.names {
        let estimate = call_with(
            &adapter.getattr("estimate_relation_row_count")?,
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
                ("name", name.into_bound_py_any(py)?),
                ("metadata_only", true.into_bound_py_any(py)?),
            ],
        )?;
        records.push(WarehouseHistoryRecord::Statistics(
            WarehouseStatisticsRecord {
                state_database: request.location.database.clone(),
                state_schema: request.location.schema.clone(),
                name: name.clone(),
                observed_at: chrono::Utc::now(),
                row_count: field(&estimate, "row_count")?.extract()?,
                detail: field(&estimate, "detail")?.extract()?,
                byte_count: field(&estimate, "byte_count")?.extract()?,
                byte_count_basis: field(&estimate, "byte_count_basis")?.extract()?,
            },
        ));
    }
    Ok(WarehouseHistoryPage {
        records,
        next_after: request.next_after.map(|id| vec![Some(id.to_string())]),
    })
}
