//! The coded error commands raise when the warehouse connection cannot be opened.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_host::entries::models::ExecutorHost;

use crate::native_host::main::host_entry::host_entry;

/// The coded error to raise for a failed `adapter.connect`, or the failure itself when coded.
pub(crate) fn connection_failure(adapter: &Bound<'_, PyAny>, error: &PyErr) -> PyErr {
    let py: Python<'_> = adapter.py();
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("adapter", adapter)?;
        options.set_item("error", error.value(py))?;
        host_entry(py, ExecutorHost::RaiseWarehouseConnectionFailure)?.call((), Some(&options))
    })();
    match raised {
        Ok(_) => error.clone_ref(py),
        Err(failure) => failure,
    }
}
