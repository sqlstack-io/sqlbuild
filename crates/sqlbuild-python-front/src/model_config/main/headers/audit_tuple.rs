//! Audit tuple.

use crate::model_config::_helpers::headers::header_metadata;
use crate::model_config::models::ContractClasses;
use crate::model_config::models::PyNode;
use pyo3::prelude::Bound;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::types::PyTuple;
use sqlbuild_model_config::header_metadata::models::ParsedAudit;

/// Audit tuple.
#[inline]
pub fn audit_tuple<'py>(
    py: Python<'py>,
    classes: &ContractClasses<'py>,
    parsed: &[ParsedAudit<PyNode<'py>>],
) -> PyResult<Bound<'py, PyTuple>> {
    header_metadata::audit_tuple(py, classes, parsed)
}
