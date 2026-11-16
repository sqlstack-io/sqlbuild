//! Native schema column.

use crate::model_config::_helpers::headers::header_metadata::ColumnFields;
use crate::model_config::_helpers::headers::header_metadata::optional;
use crate::model_config::models::ContractClasses;
use crate::model_config::models::HeaderAudits;
use crate::model_config::models::HeaderColumn;
use crate::model_config::types::HeaderValue;
use pyo3::prelude::{Bound, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_model_config::header_metadata::models::ParsedColumn;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::types::RecordId;

/// Native schema column.
pub fn native_schema_column<'py, N: HeaderValue>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    column: &ParsedColumn<N>,
    locations: &Bound<'py, PyDict>,
) -> PyResult<HeaderColumn> {
    let location = locations
        .get_item(column.name.object(py)?)?
        .unwrap_or_else(|| py.None().into_bound(py));
    let audits = HeaderAudits::new(py, classes, &column.audits, &location)?;
    let projected_audits = if classes
        .schema_column
        .is(&record_class(py, RecordId::SchemaColumn)?)
    {
        PyTuple::empty(py)
    } else {
        audits.project(py)?
    };
    let mut result = HeaderColumn::new(
        &classes.schema_column,
        [
            column.name.object(py)?,
            optional(py, column.column_type.as_ref())?,
            optional(py, column.nullable.as_ref())?,
            optional(py, column.description.as_ref())?,
            PyDict::new(py).into_any(),
            projected_audits.into_any(),
            location,
            optional(py, column.migrate_from.as_ref())?,
        ],
    )?;
    if !matches!(result.fields, ColumnFields::Projected) {
        result.audits = Some(audits);
    }
    Ok(result)
}
