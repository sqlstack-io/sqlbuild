//! A callable that forwards each completed microbatch to `callbacks`, built without a Python lookup.

use crate::build_progress::models::BuildProgressCallbacks;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyCFunction, PyDict, PyTuple, PyTupleMethods};

/// A callable that forwards each completed microbatch to `callbacks`, built without a Python lookup.
pub fn batch_complete_callback<'py>(callbacks: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let owned: Py<BuildProgressCallbacks> = callbacks
        .downcast::<BuildProgressCallbacks>()?
        .clone()
        .unbind();
    let forward = move |arguments: &Bound<'_, PyTuple>,
                        _keywords: Option<&Bound<'_, PyDict>>|
          -> PyResult<()> {
        owned
            .get()
            .on_batch_complete(arguments.py(), &arguments.get_item(0)?)
    };
    Ok(PyCFunction::new_closure(callbacks.py(), None, None, forward)?.into_any())
}
