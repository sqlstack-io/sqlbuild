//! Split unit-test selectors from resource selectors for a command that runs unit tests.

use crate::planner_selection::_helpers::unit_tests::split_python;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Split unit-test selectors from resource selectors for a command that runs unit tests.
pub fn split_unit_test_selectors<'py>(
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    split_python((select, exclude), discovered_inputs, true)
}
