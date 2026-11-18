//! The named jobs each selected node is selected through.

use std::collections::{BTreeMap, BTreeSet};

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;
use crate::graph::main::resolve_selector_tokens::resolve_selector_tokens;
use crate::graph::models::ProjectGraph;
use crate::job_settings::_helpers::tokens::token_jobs;
use crate::job_settings::errors::JobSelectorError;
use crate::job_settings::models::DeclaredRun;

fn keys(graph: &ProjectGraph, selectors: &[String]) -> Result<BTreeSet<ObjectKey>, SelectorError> {
    resolve_selector_tokens(graph, selectors).map(|keys| keys.into_iter().collect())
}

/// Each node a `--select` token selects through a job it names whose own selection holds it.
pub(crate) fn node_jobs(
    graph: &ProjectGraph,
    (select, exclude): (&[String], &[String]),
    declared: &BTreeMap<String, DeclaredRun>,
) -> Result<BTreeMap<ObjectKey, Vec<String>>, JobSelectorError> {
    let failed = |jobs: &[String], selector: &str| {
        let (jobs, selector) = (jobs.to_vec(), selector.to_owned());
        move |error: SelectorError| JobSelectorError {
            jobs,
            selector,
            error,
        }
    };
    let mut node_jobs: BTreeMap<ObjectKey, Vec<String>> = BTreeMap::new();
    let excluded: BTreeSet<ObjectKey> =
        keys(graph, exclude).map_err(failed(&[], &exclude.join(" ")))?;
    let resolved = graph.jobs.as_ref();
    for token in select
        .iter()
        .flat_map(|selector| selector.split_whitespace())
    {
        let jobs: Vec<String> = token_jobs(token, declared);
        if jobs.is_empty() {
            continue;
        }
        let selected: BTreeSet<ObjectKey> =
            keys(graph, &[token.to_owned()]).map_err(failed(&jobs, token))?;
        for job in &jobs {
            let own: &BTreeSet<ObjectKey> = match resolved.and_then(|resolved| resolved.get(job)) {
                Some(Ok(own)) => own,
                Some(Err(error)) => {
                    return Err(failed(std::slice::from_ref(job), token)(error.clone()));
                }
                None => continue,
            };
            for key in selected
                .intersection(own)
                .filter(|key| !excluded.contains(*key))
            {
                let jobs: &mut Vec<String> = node_jobs.entry(key.clone()).or_default();
                if !jobs.contains(job) {
                    jobs.push(job.clone());
                }
            }
        }
    }
    Ok(node_jobs)
}
