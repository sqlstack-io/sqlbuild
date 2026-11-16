//! Raise when a selector names a unit test; return the unchanged resource selectors.

use crate::planner_selection::_helpers::unit_tests;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Raise when a selector names a unit test; return the unchanged resource selectors.
#[inline]
pub fn reject_unit_test_selectors<'py>(
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    unit_tests::reject_unit_test_selectors(select, exclude, discovered_inputs)
}
