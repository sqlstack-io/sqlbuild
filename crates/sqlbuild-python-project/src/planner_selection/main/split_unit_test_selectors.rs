//! Split unit-test selectors from resource selectors for a command that runs unit tests.

use crate::planner_selection::_helpers::unit_tests;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Split unit-test selectors from resource selectors for a command that runs unit tests.
#[inline]
pub fn split_unit_test_selectors<'py>(
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    unit_tests::split_unit_test_selectors(select, exclude, discovered_inputs)
}
