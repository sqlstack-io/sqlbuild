//! Read statistics.

use crate::warehouse_access::_helpers::warehouse_statistics;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_orchestration::warehouse_metadata::models::StatisticsRead;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryPage;

/// Read statistics.
#[inline]
pub fn read_statistics(
    py: Python<'_>,
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    request: &StatisticsRead,
) -> PyResult<WarehouseHistoryPage> {
    warehouse_statistics::read_statistics(py, (adapter, connection), request)
}
