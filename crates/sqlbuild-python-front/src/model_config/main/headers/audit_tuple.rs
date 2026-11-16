//! Audit tuple.

use crate::model_config::_helpers::headers::header_metadata::audit_instances;
use crate::model_config::models::ContractClasses;
use crate::model_config::models::PyNode;
use pyo3::prelude::{Bound, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_model_config::header_metadata::models::ParsedAudit;

/// Audit tuple.
pub fn audit_tuple<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: &[ParsedAudit<PyNode<'py>>],
) -> PyResult<Bound<'py, PyTuple>> {
    PyTuple::new(py, audit_instances(py, classes, parsed)?)
}
