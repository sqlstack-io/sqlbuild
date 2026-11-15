//! Register records.

use crate::records::_helpers::native_classes::registration;
use crate::records::models::GetterEntry;
use crate::records::models::RecordBehaviours;
use pyo3::prelude::Bound;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyResult;

/// register_records.
#[inline]
pub fn register_records(
    module: &Bound<'_, PyModule>,
    getter_tables: &'static [&'static [GetterEntry]],
    record_behaviours: &'static RecordBehaviours,
) -> PyResult<()> {
    registration::register(module, getter_tables, record_behaviours)
}
