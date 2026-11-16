//! Return options whose selectors name only resources, with unit tests selected apart.

use crate::compile_driver::main::stages::unit_test_selectors_applied::unit_test_selectors_applied;
use pyo3::prelude::{Bound, PyAny, PyResult};
use pyo3::pyfunction;

/// Return options whose selectors name only resources, with unit tests selected apart.
#[pyfunction]
#[pyo3(signature = (*, options, discovered_inputs))]
pub fn apply_unit_test_selectors<'py>(
    options: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    unit_test_selectors_applied(options, discovered_inputs)
}
