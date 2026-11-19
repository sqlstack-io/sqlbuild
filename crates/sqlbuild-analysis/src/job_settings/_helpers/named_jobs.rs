//! The jobs a selection names, as `job:<name>` or a bare job name; naming a group names its jobs.

use std::collections::BTreeMap;

use crate::job_settings::_helpers::tokens::token_jobs;
use crate::job_settings::models::DeclaredRun;

/// The jobs `selectors` name, each once in the order first named.
#[must_use]
pub(crate) fn named_jobs(
    selectors: &[String],
    declared: &BTreeMap<String, DeclaredRun>,
) -> Vec<String> {
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
