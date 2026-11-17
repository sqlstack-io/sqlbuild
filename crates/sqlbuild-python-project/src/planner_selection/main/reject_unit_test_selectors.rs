//! Raise when a selector names a unit test; return the unchanged resource selectors.

use crate::planner_selection::_helpers::unit_tests::split_python;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Raise when a selector names a unit test; return the unchanged resource selectors.
pub fn reject_unit_test_selectors<'py>(
    select: &Bound<'py, PyAny>,
    exclude: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    split_python((select, exclude), discovered_inputs, false)
}
