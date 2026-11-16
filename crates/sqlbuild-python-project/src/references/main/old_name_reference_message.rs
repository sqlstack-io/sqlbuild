//! The P008 message for SQL naming a relation kept only as a compatibility view.

use crate::references::_helpers::explicit_references;

/// The P008 message for SQL naming a relation kept only as a compatibility view.
#[inline]
pub fn old_name_reference_message(
    owner_label: &str,
    written: &str,
    method: &str,
    model_name: &str,
) -> String {
    explicit_references::old_name_reference_message(owner_label, written, method, model_name)
}
