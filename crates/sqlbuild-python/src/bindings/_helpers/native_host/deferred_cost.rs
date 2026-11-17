//! A worker build's cost finalization, held until the run has published its outcome.

use std::sync::Mutex;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyResult};

/// The one finalization a worker build defers, if any.
#[derive(Debug, Default)]
pub(crate) struct DeferredCost(Mutex<Option<Py<PyAny>>>);

fn poisoned() -> PyErr {
    PyRuntimeError::new_err("deferred cost lock is poisoned")
}

impl DeferredCost {
    /// Keep `finalization`, replacing any earlier one.
    pub(crate) fn defer(&self, finalization: &Bound<'_, PyAny>) -> PyResult<()> {
        *self.0.lock().map_err(|_| poisoned())? = Some(finalization.clone().unbind());
        Ok(())
    }

    /// The deferred finalization, once.
    pub(crate) fn take(&self) -> PyResult<Option<Py<PyAny>>> {
        Ok(self.0.lock().map_err(|_| poisoned())?.take())
    }
}
