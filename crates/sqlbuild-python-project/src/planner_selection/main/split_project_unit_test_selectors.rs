//! Move unit-test selectors out of resource selectors, or reject them for this command.

use crate::planner_selection::_helpers::unit_tests;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;

/// Move unit-test selectors out of resource selectors, or reject them for this command.
#[inline]
pub fn split_project_unit_test_selectors<'py>(
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
    accepts_unit_tests: bool,
) -> PyResult<Bound<'py, PyAny>> {
    unit_tests::split_project_unit_test_selectors(
        select,
        exclude,
        discovered_inputs,
        accepts_unit_tests,
    )
}
