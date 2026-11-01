//! Rules results rendered for the CLI, and SQLCLEAN044 help naming files that repeat a literal.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_rules::rule_results::main::format_result::format_result;
use sqlbuild_rules::rule_results::main::format_rules_progress::format_rules_progress;
use sqlbuild_rules::rule_results::main::format_skipped_rules::format_skipped_rules;
use sqlbuild_rules::rule_results::models::{ResultCounts, RuleFinding};

use crate::bindings::_helpers::records::fields::{field, path_posix};

pub(crate) fn rule_finding(finding: &Bound<'_, PyAny>) -> PyResult<RuleFinding> {
    Ok(RuleFinding {
        code: field(finding, "code")?.extract()?,
        path: path_posix(&field(finding, "path")?)?,
        line: field(finding, "line")?.extract()?,
        column: field(finding, "column")?.extract()?,
        message: field(finding, "message")?.extract()?,
        remediation: field(finding, "remediation")?.extract()?,
        unevaluated: field(finding, "unevaluated")?.is_truthy()?,
        affected_rules: field(finding, "affected_rules")?.extract()?,
        fixable: field(finding, "fixable")?.is_truthy()?,
    })
}

fn rule_findings(findings: &Bound<'_, PyAny>) -> PyResult<Vec<RuleFinding>> {
    findings
        .try_iter()?
        .map(|finding| rule_finding(&finding?))
        .collect()
}

/// One rules result for the requested CLI surface.
#[pyfunction]
#[pyo3(signature = (*, result, json_output))]
pub(crate) fn format_rules_result(
    result: &Bound<'_, PyAny>,
    json_output: bool,
) -> PyResult<String> {
    let counts = ResultCounts {
        evaluated_models: field(result, "evaluated_models")?.extract()?,
        unevaluated_resources: field(result, "unevaluated_resources")?.extract()?,
        cache_hits: field(result, "cache_hits")?.extract()?,
        cache_misses: field(result, "cache_misses")?.extract()?,
    };
    Ok(format_result(
        &rule_findings(&field(result, "findings")?)?,
        counts,
        json_output,
    ))
}

/// The rules phase completion line for one `RulesRunResult`, optionally noting skipped rules.
pub(crate) fn rules_progress(
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

/// Describe type-proof rules skipped because SQL analysis is disabled, if any.
#[pyfunction]
#[pyo3(signature = (*, codes))]
pub(crate) fn format_skipped_type_proof_rules(codes: Vec<String>) -> Option<String> {
    format_skipped_rules(&codes)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(format_rules_result, module)?)?;
    module.add_function(wrap_pyfunction!(format_skipped_type_proof_rules, module)?)?;
    Ok(())
}
