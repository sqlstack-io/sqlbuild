//! A `SqlSchemaValidationRequest` over the normalized analysis SQL of `parts.query_sql`.

use crate::analysis::_helpers::schema_request;
use crate::analysis::models::SchemaRequestParts;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// A `SqlSchemaValidationRequest` over the normalized analysis SQL of `parts.query_sql`.
#[inline]
pub fn schema_validation_request<'py>(
    py: Python<'py>,
    parts: SchemaRequestParts<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    schema_request::schema_validation_request(py, parts)
}
