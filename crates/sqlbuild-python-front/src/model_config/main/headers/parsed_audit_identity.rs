//! `audit.name or audit.definition_name` of the instance `audit` builds.

use crate::model_config::_helpers::headers::header_metadata;
use crate::model_config::types::HeaderValue;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_model_config::header_metadata::models::ParsedAudit;

/// `audit.name or audit.definition_name` of the instance `audit` builds.
#[inline]
pub fn parsed_audit_identity<N: HeaderValue>(
    py: Python<'_>,
    audit: &ParsedAudit<N>,
) -> PyResult<String> {
    header_metadata::parsed_audit_identity(py, audit)
}
