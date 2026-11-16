//! `json.dumps(value, sort_keys=True, separators=separators)`, with `default=str` when `default_str`.

use crate::planner_identity::_helpers::json_values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// `json.dumps(value, sort_keys=True, separators=separators)`, with `default=str` when `default_str`.
#[inline]
pub fn sorted_dumps(
    value: &Bound<'_, PyAny>,
    separators: (&str, &str),
    default_str: bool,
) -> PyResult<String> {
    json_values::sorted_dumps(value, separators, default_str)
}
