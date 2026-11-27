//! A failed test or scenario as its `Failures:` entry and the short reason of its `Failed (N):` line.

use crate::run_output::main::short_reason::short_reason;
use crate::run_output::models::{FailureEntry, FailureField};
use crate::test_command::_helpers::failure_rows::{step_fields, step_reason};
use crate::test_command::constants::{DEFAULT_ERROR, EXPECTATIONS_FAILED_CODES, TIMED_OUT_REASON};
use crate::test_command::models::TestFailureFacts;

/// The entry, with the error, any failing expectation's rows, the statement and the files, and the short reason.
#[must_use]
pub fn test_failure_entry(facts: &TestFailureFacts) -> (FailureEntry, String) {
    let mut meta: Vec<String> = vec![facts.kind.clone()];
    if !facts.run_on.is_empty() {
        meta.push(facts.run_on.clone());
    }
    let mut fields: Vec<FailureField> = Vec::new();
    let mut reason: String = String::new();
    for step in &facts.steps {
        let (step_lines, step_why) = (step_fields(step), step_reason(step));
        if !step_lines.is_empty() && reason.is_empty() {
            reason = step_why;
        }
        fields.extend(step_lines);
    }
    let message: Option<&str> = facts
        .error_message
        .as_deref()
        .filter(|message| !message.is_empty())
        .filter(|_| {
            fields.is_empty()
                || !facts
                    .error_code
                    .as_deref()
                    .is_some_and(|code| EXPECTATIONS_FAILED_CODES.contains(&code))
        });
    if message.is_some() || fields.is_empty() {
        let message: &str = message.unwrap_or(DEFAULT_ERROR);
        fields.insert(
            0,
            FailureField::Error {
                code: facts.error_code.clone(),
                message: message.to_owned(),
                help: facts.error_help.clone(),
            },
        );
        reason = if facts.timed_out {
            TIMED_OUT_REASON.to_owned()
        } else {
            short_reason(facts.error_code.as_deref(), message)
        };
    }
    if let Some((phase, kind)) = &facts.statement {
        fields.push(FailureField::Labelled {
            label: "statement".to_owned(),
            text: format!("{phase} \u{00b7} {kind}"),
        });
        if let Some(path) = &facts.compiled_path {
            fields.push(FailureField::Verbatim(path.clone()));
        }
    }
    if let Some(path) = &facts.source_path {
        fields.push(FailureField::Labelled {
            label: facts.kind.clone(),
            text: path.clone(),
        });
    }
    (
        FailureEntry {
            name: facts.name.clone(),
            meta: meta.join(", "),
            fields,
        },
        reason,
    )
}
