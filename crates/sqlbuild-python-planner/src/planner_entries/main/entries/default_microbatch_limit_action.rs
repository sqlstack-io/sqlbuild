//! The microbatch limit action used when a project configures none.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::types::RecordId;

/// The microbatch limit action used when a project configures none.
pub fn default_microbatch_limit_action(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    enum_member(py, RecordId::MicrobatchLimitAction, "ERROR")
}
