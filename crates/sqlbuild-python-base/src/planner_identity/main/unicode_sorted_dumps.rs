//! `json.dumps(value, ensure_ascii=False, sort_keys=True, separators=separators)`.

use crate::planner_identity::_helpers::json_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `json.dumps(value, ensure_ascii=False, sort_keys=True, separators=separators)`.
#[inline]
pub fn unicode_sorted_dumps(
    value: &Bound<'_, PyAny>,
    separators: (&str, &str),
) -> PyResult<String> {
    json_values::unicode_sorted_dumps(value, separators)
}
