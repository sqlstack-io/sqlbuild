//! `audit.name or audit.definition_name` of the instance `audit` builds.

use crate::model_config::types::HeaderValue;
use pyo3::prelude::{PyAnyMethods, PyResult, Python};
use sqlbuild_model_config::header_metadata::models::ParsedAudit;

/// `audit.name or audit.definition_name` of the instance `audit` builds.
pub fn parsed_audit_identity<N: HeaderValue>(
    py: Python<'_>,
    audit: &ParsedAudit<N>,
) -> PyResult<String> {
    if let Some(name) = &audit.name
        && name.object(py)?.is_truthy()?
    {
        return name.exact_text(py);
    }
    audit.definition_name.exact_text(py)
}
