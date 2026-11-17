//! `audit_resource_id` of one audit result or plan entry, whose name is in `audit_name_field`.

use crate::result_output::_helpers::events::resource_ids::audit_identity_of;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// `audit_resource_id` of one audit result or plan entry, whose name is in `audit_name_field`.
pub fn audit_identity(result: &Bound<'_, PyAny>, audit_name_field: &str) -> PyResult<String> {
    let target_kind: Bound<'_, PyAny> = native_field(result, "attached_target_kind")?;
    let kind: Bound<'_, PyAny> = if target_kind.is_none() {
        native_field(result, "attachment_kind")?
    } else {
        target_kind
    };
    audit_identity_of(
        &native_field(result, audit_name_field)?,
        &kind,
        (
            &native_field(result, "attached_target_name")?,
            &native_field(result, "attached_column_name")?,
        ),
    )
}
