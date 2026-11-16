//! `sql` expanded with lint's expansion inputs, as `file_path`'s declarations see them.

use crate::render::_helpers::declarations::authored_sql;
use crate::render::models::AuthoredExpansion;
use crate::render::models::SqlExpansionInputs;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `sql` expanded with lint's expansion inputs, as `file_path`'s declarations see them.
#[inline]
pub fn expand_with_inputs(
    inputs: &SqlExpansionInputs,
    sql: &str,
    file_path: &Bound<'_, PyAny>,
) -> PyResult<AuthoredExpansion> {
    authored_sql::expand_with_inputs(inputs, sql, file_path)
}
