//! The explicit effective batch-size token resolved against the replay grain.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{PyAnyMethods, PyErr, Python};
use pyo3::pyfunction;
use sqlbuild_core::plan_display::main::effective_microbatch_batch_size::effective_microbatch_batch_size;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::types::RecordId;

/// The explicit effective batch-size token resolved against the replay grain.
#[pyfunction]
#[pyo3(signature = (*, batch_size, effective_grain))]
pub fn resolve_effective_microbatch_batch_size(
    py: Python<'_>,
    batch_size: &str,
    effective_grain: &str,
) -> Result<String, PyErr> {
    effective_microbatch_batch_size(batch_size, effective_grain).map_err(|error| {
        record_class(py, RecordId::CursorGrain)
            .and_then(|grain| grain.call1((error.0,)))
            .err()
            .unwrap_or_else(|| PyValueError::new_err("unknown cursor grain"))
    })
}
