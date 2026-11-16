//! `macro_source_sql` with each call of a mocked SQL macro the model makes replaced by its mock.

use crate::render::_helpers::sql_macros::sql_macro_mocks;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::types::PyDict;

/// `macro_source_sql` with each call of a mocked SQL macro the model makes replaced by its mock.
#[inline]
pub fn with_sql_macro_mocks(
    model_input: &Bound<'_, PyAny>,
    macro_source_sql: String,
    (mocks, loaded_macros): (&Bound<'_, PyDict>, &Bound<'_, PyAny>),
) -> PyResult<String> {
    sql_macro_mocks::with_sql_macro_mocks(model_input, macro_source_sql, (mocks, loaded_macros))
}
