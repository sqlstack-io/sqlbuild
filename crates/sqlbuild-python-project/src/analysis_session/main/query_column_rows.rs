//! Query column rows.

use crate::analysis_session::_helpers::stage_functions;
use crate::analysis_session::types::ColumnsRow;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Query column rows.
#[inline]
pub fn query_column_rows(
    queries: &Bound<'_, PyAny>,
    profile: &Bound<'_, PyAny>,
    (types, nullability): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<Vec<ColumnsRow>> {
    stage_functions::query_column_rows(queries, profile, (types, nullability))
}
