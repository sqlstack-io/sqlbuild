//! `(has star, star resolved)` of the model's analysed query.

use crate::compiled_views::_helpers::compiled_models::model_reads;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `(has star, star resolved)` of the model's analysed query.
#[inline]
pub fn star_flags(model: &Bound<'_, PyAny>) -> PyResult<(bool, bool)> {
    model_reads::star_flags(model)
}
