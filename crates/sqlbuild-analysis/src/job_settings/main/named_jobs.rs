//! The jobs a selection names, as `job:<name>` or a bare job name; naming a group names its jobs.

use std::collections::BTreeMap;

use crate::graph::_helpers::selectors::parse;
use crate::graph::constants::{INTERSECTION_SEPARATOR, JOB_KIND, NAME_KIND};
use crate::graph::models::ParsedSelector;
use crate::job_settings::models::DeclaredRun;

/// The jobs one whitespace-free selector token names, each once in the order named.
#[must_use]
pub fn token_jobs(token: &str, declared: &BTreeMap<String, DeclaredRun>) -> Vec<String> {
    let mut jobs: Vec<String> = Vec::new();
    for part in token.split(INTERSECTION_SEPARATOR) {
        let Ok(ParsedSelector::Kind { kind, value, .. }) = parse(part) else {
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

/// The jobs `selectors` name, each once in the order first named.
#[must_use]
pub fn named_jobs(selectors: &[String], declared: &BTreeMap<String, DeclaredRun>) -> Vec<String> {
    let mut jobs: Vec<String> = Vec::new();
    for token in selectors
        .iter()
        .flat_map(|selector| selector.split_whitespace())
    {
        for job in token_jobs(token, declared) {
            if !jobs.contains(&job) {
                jobs.push(job);
            }
        }
    }
    jobs
}
