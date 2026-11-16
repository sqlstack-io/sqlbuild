//! Read catalog.

use crate::warehouse_access::_helpers::warehouse_catalog;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_orchestration::warehouse_metadata::models::CatalogRead;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseHistoryPage;

/// Read catalog.
#[inline]
pub fn read_catalog(
    py: Python<'_>,
    (adapter, connection): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    request: &CatalogRead,
) -> PyResult<WarehouseHistoryPage> {
    warehouse_catalog::read_catalog(py, (adapter, connection), request)
}
