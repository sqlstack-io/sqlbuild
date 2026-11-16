//! `watermark_type_is_compatible` as a function of `module`.

use crate::planner_warehouse::_helpers::inspection::watermarks;

/// `watermark_type_is_compatible` as a function of `module`.
#[inline]
pub fn watermark_type_is_compatible_function<'py>(
    module: &pyo3::Bound<'py, pyo3::types::PyModule>,
) -> pyo3::PyResult<pyo3::Bound<'py, pyo3::types::PyCFunction>> {
    watermarks::watermark_type_is_compatible_function(module)
}
