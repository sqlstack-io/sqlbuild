//! The jobs one selector token names.

use std::collections::BTreeMap;

use sqlbuild_core::text::main::glob_match::glob_match;

use crate::graph::constants::{INTERSECTION_SEPARATOR, JOB_KIND, NAME_KIND, PATTERN_CHARACTERS};
use crate::graph::main::parse_selector::parse_selector;
use crate::graph::models::ParsedSelector;
use crate::job_settings::models::DeclaredRun;

/// The jobs one selector token names, by name, `job:` pattern or group, each once.
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
        let pattern: bool = kind == JOB_KIND && value.contains(PATTERN_CHARACTERS);
        let named = declared
            .iter()
            .filter(|(name, _)| **name == value || (pattern && glob_match(name, &value)))
            .flat_map(|(name, run)| match run {
                DeclaredRun::Job { .. } => vec![name.clone()],
                DeclaredRun::Group { members } => members.clone(),
            });
        for job in named {
            if !jobs.contains(&job) {
                jobs.push(job);
            }
        }
    }
    jobs
}
