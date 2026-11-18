//! The error a plan preview raises for a warehouse read it refuses.

use pyo3::create_exception;
use pyo3::exceptions::PyRuntimeError;

create_exception!(
    sqlbuild._native,
    WarehouseAccessRefusedError,
    PyRuntimeError,
    "Raised when a plan preview reaches past the synced warehouse state for the warehouse."
);
