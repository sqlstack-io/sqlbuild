//! Forward a Python check lifecycle callback directly to its native progress owner.

use crate::build_progress::models::BuildProgressCallbacks;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyCFunction, PyDict, PyTuple, PyTupleMethods};

/// Forward a Python check lifecycle callback directly to its native progress owner.
pub fn check_callback<'py>(
    callbacks: &Bound<'py, PyAny>,
    completion: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let owned: Py<BuildProgressCallbacks> = callbacks
        .downcast::<BuildProgressCallbacks>()?
        .clone()
        .unbind();
    let forward = move |arguments: &Bound<'_, PyTuple>,
                        _keywords: Option<&Bound<'_, PyDict>>|
          -> PyResult<()> {
        let value = arguments.get_item(0)?;
        if completion {
            owned.get().on_check_complete(arguments.py(), &value)
        } else {
            owned.get().on_check_start(arguments.py(), value.extract()?)
        }
    };
    Ok(PyCFunction::new_closure(callbacks.py(), None, None, forward)?.into_any())
}
