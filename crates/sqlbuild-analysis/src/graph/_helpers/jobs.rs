//! Jobs and job groups resolved once on the graph, so `job:` selects what the job publishes.

use std::collections::BTreeMap;

use crate::graph::_helpers::selectors::{Keys, resolve};
use crate::graph::constants::UNKNOWN_NAME_CODE;
use crate::graph::errors::SelectorError;
use crate::graph::models::{JobSelection, ProjectGraph};

/// Resolve every job on the graph without jobs attached, as publication does, then attach them.
pub(crate) fn attach(
    mut graph: ProjectGraph,
    catalog: BTreeMap<String, JobSelection>,
) -> ProjectGraph {
    graph.jobs = None;
    let mut resolved: BTreeMap<String, Result<Keys, SelectorError>> = BTreeMap::new();
    for (name, selection) in &catalog {
        if let JobSelection::Job { select, exclude } = selection {
            let keys = resolve(&graph, select, exclude).map(|keys| keys.into_iter().collect());
            resolved.insert(name.clone(), keys);
        }
    }
    for (name, selection) in &catalog {
        if let JobSelection::Group { members } = selection {
            let mut keys: Keys = Keys::new();
            let mut failure: Option<SelectorError> = None;
            for member in members {
                match resolved.get(member) {
                    Some(Ok(member_keys)) => keys.extend(member_keys.iter().cloned()),
                    Some(Err(error)) => failure = failure.or_else(|| Some(error.clone())),
                    None => {
                        failure = failure.or_else(|| {
                            Some(SelectorError {
                                code: UNKNOWN_NAME_CODE,
                                message: format!("job group '{name}' names unknown job '{member}'"),
                                help: None,
                            })
                        });
                    }
                }
            }
            resolved.insert(name.clone(), failure.map_or(Ok(keys), Err));
        }
    }
    graph.jobs = Some(resolved);
    graph
}
