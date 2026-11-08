//! `lint_sql_bodies`: native SQL lint of expanded bodies, mapped onto authored source.

use std::collections::HashMap;

use serde_json::Value;

use crate::lint_text::_helpers::sql_diagnostics::{
    AuthoredFile, LintResult, body_diagnostics, response_diagnostics, unavailable_once_per_file,
};
use crate::lint_text::_helpers::sql_requests::{lint_payload, omitted_select, run_batch};
use crate::lint_text::models::{AuthoredDiagnostic, LintSqlBody, LintSqlOptions};

/// Lint each distinct request once and map its diagnostics, or one unavailable warning per file, onto source.
pub fn lint_sql_bodies(
    bodies: &[LintSqlBody],
    files: &[&str],
    options: &LintSqlOptions,
) -> Vec<AuthoredDiagnostic> {
    let omitted: Vec<Option<usize>> = bodies.iter().map(omitted_select).collect();
    let mut payloads: Vec<Value> = Vec::new();
    let mut index_by_request: HashMap<String, usize> = HashMap::new();
    let mut request_of_body: Vec<usize> = Vec::with_capacity(bodies.len());
    for (body, offset) in bodies.iter().zip(&omitted) {
        let payload: Value = lint_payload((body, files[body.file]), *offset, options);
        let key: String = payload.to_string();
        let next: usize = payloads.len();
        let index: usize = *index_by_request.entry(key).or_insert(next);
        if index == next {
            payloads.push(payload);
        }
        request_of_body.push(index);
    }
    let responses: Vec<Result<LintResult, String>> = run_batch(&payloads)
        .map(|responses| responses.iter().map(response_diagnostics).collect())
        .unwrap_or_else(|error| vec![Err(error); payloads.len()]);
    let authored: Vec<AuthoredFile<'_>> = files
        .iter()
        .map(|text| AuthoredFile::new(text, &options.dialect))
        .collect();
    let diagnostics: Vec<AuthoredDiagnostic> = bodies
        .iter()
        .zip(omitted)
        .zip(request_of_body)
        .flat_map(|((body, offset), request)| {
            body_diagnostics(&responses[request], (body, offset), &authored[body.file])
        })
        .collect();
    unavailable_once_per_file(diagnostics)
}
