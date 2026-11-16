//! What the analysed SQL of a native model input must show for its SQL macro calls to fit.

use crate::compiled_views::_helpers::model_inputs::model_input_view;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::sql_macros::models::SqlMacroCheck;

/// What the analysed SQL of a native model input must show for its SQL macro calls to fit.
#[inline]
pub fn input_sql_macro_checks(model_input: &Bound<'_, PyAny>) -> PyResult<Vec<SqlMacroCheck>> {
    model_input_view::input_sql_macro_checks(model_input)
}
