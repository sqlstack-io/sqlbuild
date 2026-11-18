//! The jobs one selector token names.

use std::collections::BTreeMap;

use crate::graph::constants::{INTERSECTION_SEPARATOR, JOB_KIND, NAME_KIND};
use crate::graph::main::parse_selector::parse_selector;
use crate::graph::models::ParsedSelector;
use crate::job_settings::models::DeclaredRun;

/// The jobs one whitespace-free selector token names, each once in the order named.
#[must_use]
pub(crate) fn token_jobs(token: &str, declared: &BTreeMap<String, DeclaredRun>) -> Vec<String> {
    let mut jobs: Vec<String> = Vec::new();
    for part in token.split(INTERSECTION_SEPARATOR) {
        let Ok(ParsedSelector::Kind { kind, value, .. }) = parse_selector(part) else {
            continue;
        };
        if kind != JOB_KIND && kind != NAME_KIND {
            continue;
        }
        let named: Vec<String> = match declared.get(&value) {
            Some(DeclaredRun::Job { .. }) => vec![value],
            Some(DeclaredRun::Group { members }) => members.clone(),
            None => Vec::new(),
        };
        for job in named {
            if !jobs.contains(&job) {
                jobs.push(job);
            }
        }
    }
    jobs
}
