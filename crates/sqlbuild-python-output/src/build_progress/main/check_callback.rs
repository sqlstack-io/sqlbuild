//! Forward a Python check lifecycle callback directly to its native progress owner.

use crate::build_progress::_helpers::build_callbacks::relay;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::PyTupleMethods;

/// Forward a Python check lifecycle callback directly to its native progress owner.
pub fn check_callback<'py>(
    callbacks: &Bound<'py, PyAny>,
    completion: bool,
) -> PyResult<Bound<'py, PyAny>> {
    if completion {
        return relay(callbacks, |owner, arguments| {
            owner.on_check_complete(arguments.py(), &arguments.get_item(0)?)
        });
    }
    relay(callbacks, |owner, arguments| {
        owner.on_check_start(arguments.py(), arguments.get_item(0)?.extract()?)
    })
}
