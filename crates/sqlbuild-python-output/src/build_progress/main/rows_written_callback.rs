//! A callable that forwards a running loader's rows so far to `callbacks`, built without a Python lookup.

use crate::build_progress::models::BuildProgressCallbacks;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyCFunction, PyDict, PyTuple, PyTupleMethods};

/// A callable taking a node key and its rows so far, which a worker reports as the node's progress.
pub fn rows_written_callback<'py>(callbacks: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let owned: Py<BuildProgressCallbacks> = callbacks
        .downcast::<BuildProgressCallbacks>()?
        .clone()
        .unbind();
    let forward = move |arguments: &Bound<'_, PyTuple>,
                        _keywords: Option<&Bound<'_, PyDict>>|
          -> PyResult<()> {
        let key: String = arguments.get_item(0)?.extract()?;
        let rows: i64 = arguments.get_item(1)?.extract()?;
        owned.get().on_rows_written(arguments.py(), &key, rows)
    };
    Ok(PyCFunction::new_closure(callbacks.py(), None, None, forward)?.into_any())
}
