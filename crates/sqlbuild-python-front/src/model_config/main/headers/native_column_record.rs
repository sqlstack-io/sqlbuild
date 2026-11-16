//! The `SchemaColumn` a native column projects to when its location names `location`'s path.

use crate::model_config::_helpers::headers::header_metadata::column_values;
use crate::model_config::_helpers::headers::header_metadata::contract_object;
use crate::model_config::_helpers::headers::header_metadata::location_record;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_discovery::model_files::models::NativeColumn;
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::types::RecordId;

/// The `SchemaColumn` a native column projects to when its location names `location`'s path.
pub fn native_column_record<'py>(
    py: Python<'py>,
    column: &NativeColumn,
    location: Option<(Bound<'py, PyAny>, LineColumnSpan)>,
) -> PyResult<Bound<'py, PyAny>> {
    let location: Bound<'py, PyAny> = match location {
        Some((path, span)) => location_record(py, path, span)?,
        None => py.None().into_bound(py),
    };
    contract_object(
        &record_class(py, RecordId::SchemaColumn)?,
        column_values(py, column, location)?,
    )
}
