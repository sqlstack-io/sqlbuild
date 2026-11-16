//! `ignores_quoted_case`: whether the connection explicitly enables case-insensitive quoting.

use crate::analysis::_helpers::binding_catalog;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `ignores_quoted_case`: whether the connection explicitly enables case-insensitive quoting.
#[inline]
pub fn ignores_quoted_case(
    connection: &Bound<'_, PyAny>,
    dialect: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    binding_catalog::ignores_quoted_case(connection, dialect)
}
