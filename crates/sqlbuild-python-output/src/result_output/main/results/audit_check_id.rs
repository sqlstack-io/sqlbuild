//! `audit_resource_id` of one audit result.

use crate::result_output::main::events::audit_identity::audit_identity;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// `audit_resource_id` of one audit result.
pub fn audit_check_id(result: &Bound<'_, PyAny>) -> PyResult<String> {
    audit_identity(result, "audit_name")
}
