//! A relay calling `target` with `names` as keywords.

use crate::build_command::_helpers::support;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// A relay calling `target` with `names` as keywords.
#[inline]
pub fn keyword_relay<'py>(
    target: Bound<'py, PyAny>,
    names: &[&'static str],
) -> PyResult<Bound<'py, PyAny>> {
    support::keyword_relay(target, names)
}
