//! The P008 message for SQL naming a relation kept only as a compatibility view.

use pyo3::pyfunction;
use sqlbuild_analysis::explicit_references::main::old_name_message::old_name_message;

/// The P008 message for SQL naming a relation kept only as a compatibility view.
#[pyfunction]
#[pyo3(signature = (*, owner_label, written, method, model_name))]
pub fn old_name_reference_message(
    owner_label: &str,
    written: &str,
    method: &str,
    model_name: &str,
) -> String {
    old_name_message(owner_label, written, method, model_name)
}
