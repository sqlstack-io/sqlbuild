//! The value `json.dumps` encodes, copied by `asdict` first when `dataclasses` is set.

use crate::python_values::_helpers::values;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_analysis::python_values::models::PythonValue;

/// The value `json.dumps` encodes, copied by `asdict` first when `dataclasses` is set.
#[inline]
pub fn python_value(value: &Bound<'_, PyAny>, dataclasses: bool) -> PyResult<PythonValue> {
    values::python_value(value, dataclasses)
}
