//! The analysis text of `model_input`: its compiled query with each typed SQL macro call marked.

use crate::compiled_views::_helpers::model_inputs::model_input_view;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// The analysis text of `model_input`: its compiled query with each typed SQL macro call marked.
#[inline]
pub fn input_analysis_sql(model_input: &Bound<'_, PyAny>, dialect: &str) -> PyResult<String> {
    model_input_view::input_analysis_sql(model_input, dialect)
}
