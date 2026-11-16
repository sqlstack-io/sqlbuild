//! The rules phase completion line for one `RulesRunResult`, optionally noting skipped rules.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_rules::rule_results::main::format_rules_progress::format_rules_progress;

/// The rules phase completion line for one `RulesRunResult`, optionally noting skipped rules.
pub fn rules_progress(
    elapsed_seconds: f64,
    result: &Bound<'_, PyAny>,
    note_skipped_rules: bool,
) -> PyResult<String> {
    let skipped: Option<Vec<String>> = if note_skipped_rules {
        Some(field(result, "skipped_type_proof_rules")?.extract()?)
    } else {
        None
    };
    Ok(format_rules_progress(
        elapsed_seconds,
        (
            field(result, "built_in_ms")?.extract()?,
            field(result, "custom_ms")?.extract()?,
        ),
        skipped.as_deref(),
    ))
}
