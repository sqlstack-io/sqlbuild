//! The header before a run of tests or scenarios: how many run where, and the causes.

use crate::run_on::_helpers::explain_rows::refusal_explain_rows;
use crate::run_on::main::refusal_causes::refusal_causes;
use crate::run_on::models::{RunOn, RunOnDecision};
use crate::run_output::models::RunHeader;

/// The header for `decisions` under `mode`, titled `title`; `--explain`'s rows only in `auto`.
#[must_use]
pub fn run_on_header(
    decisions: &[RunOnDecision],
    (mode, explain): (RunOn, bool),
    (title, (singular, plural)): (&str, (&str, &str)),
) -> RunHeader {
    let local_only: bool = mode == RunOn::Local;
    let refused: usize = decisions
        .iter()
        .filter(|decision| !decision.refusals.is_empty())
        .count();
    let local: usize = decisions
        .iter()
        .filter(|decision| decision.local() && decision.refusals.is_empty())
        .count();
    let not_run: usize = refused * usize::from(local_only);
    RunHeader {
        title: title.to_owned(),
        nouns: (singular.to_owned(), plural.to_owned()),
        selected: decisions.len(),
        local,
        warehouse: decisions.len() - local - not_run,
        not_run,
        local_only,
        causes: match mode {
            RunOn::Warehouse => Vec::new(),
            RunOn::Local | RunOn::Auto => refusal_causes(decisions),
        },
        explain: match (mode, explain) {
            (RunOn::Auto, true) => refusal_explain_rows(decisions),
            _ => Vec::new(),
        },
    }
}
