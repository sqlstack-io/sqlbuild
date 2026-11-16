//! Describe type-proof rules skipped because SQL analysis is disabled, if any.

use pyo3::pyfunction;
use sqlbuild_rules::rule_results::main::format_skipped_rules::format_skipped_rules;

/// Describe type-proof rules skipped because SQL analysis is disabled, if any.
#[pyfunction]
#[pyo3(signature = (*, codes))]
pub fn format_skipped_type_proof_rules(codes: Vec<String>) -> Option<String> {
    format_skipped_rules(&codes)
}
