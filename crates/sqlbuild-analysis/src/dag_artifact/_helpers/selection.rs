//! Closures, directed path nodes and selector resolution over string-keyed DAG edges.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;

use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;
use sqlbuild_core::graph_walk::main::directed_path_nodes::directed_path_nodes;
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;

use crate::dag_artifact::constants::{
    SELECTOR_KIND_CHECK, SELECTOR_KIND_JOB, SELECTOR_KIND_NAME, SELECTOR_KIND_PATH,
    SELECTOR_KIND_SEPARATOR, SELECTOR_KIND_TAG, SELECTOR_KIND_TEST, SELECTOR_MODELS_PREFIX,
};
use crate::dag_artifact::errors::DagSelectionError;
use crate::dag_artifact::models::{DagSelectionIndex, DagSelectionNode};
use crate::graph::main::parse_selector::parse_selector;
use crate::graph::models::ParsedSelector;

/// Upstream and downstream neighbours of each node id.
pub(crate) type Adjacency = BTreeMap<String, BTreeSet<String>>;

/// Every id reachable from `starts`, plus the starts when `include_starts`.
pub(crate) fn closure(
    starts: &BTreeSet<String>,
    edges: &Adjacency,
    include_starts: bool,
) -> BTreeSet<String> {
    let mut visited: BTreeSet<&String> = BTreeSet::new();
    if include_starts {
        visited.extend(starts);
    }
    let Ok(()) = breadth_first_reach(starts.iter(), None, &mut visited, |current| {
        Ok::<_, Infallible>(edges.get(*current).into_iter().flatten())
    });
    visited.into_iter().cloned().collect()
}

/// Ids on directed paths from `start` to `end`, or `None` when `end` is not downstream.
pub(crate) fn path_nodes(
    (start, end): (&str, &str),
    (upstream, downstream): (&Adjacency, &Adjacency),
) -> Option<BTreeSet<String>> {
    let (start, end): (String, String) = (start.to_owned(), end.to_owned());
    let (mut reachable, mut path): (BTreeSet<&String>, BTreeSet<&String>) =
        (BTreeSet::new(), BTreeSet::new());
    let Ok(found) = directed_path_nodes(
        (&start, &end),
        (
            |current: &&String| Ok(downstream.get(*current).into_iter().flatten()),
            |current: &&String| Ok(upstream.get(*current).into_iter().flatten()),
        ),
        (&mut reachable, &mut path),
    );
    found.then(|| path.into_iter().cloned().collect())
}

pub(crate) fn resolve_expression(
    index: &DagSelectionIndex<'_>,
    expression: &str,
) -> Result<BTreeSet<String>, DagSelectionError> {
    let mut selected: BTreeSet<String> = BTreeSet::new();
    for token in expression
        .split(python_isspace)
        .filter(|token| !token.is_empty())
    {
        let mut token_selection: Option<BTreeSet<String>> = None;
        for part in token.split(',') {
            let matched: BTreeSet<String> = resolve_atomic(index, part)?;
            token_selection = Some(match token_selection {
                None => matched,
                Some(previous) => previous.intersection(&matched).cloned().collect(),
            });
        }
        selected.extend(token_selection.unwrap_or_default());
    }
    Ok(selected)
}

fn resolve_atomic(
    index: &DagSelectionIndex<'_>,
    raw: &str,
) -> Result<BTreeSet<String>, DagSelectionError> {
    match parse_selector(raw).map_err(DagSelectionError::Selector)? {
        ParsedSelector::Path {
            start,
            end,
            upstream,
            downstream,
        } => {
            let start_id: String = node_id_for_name(index, &start)?;
            let end_id: String = node_id_for_name(index, &end)?;
            let mut matched: BTreeSet<String> =
                path_nodes((&start_id, &end_id), (&index.upstream, &index.downstream)).ok_or_else(
                    || {
                        DagSelectionError::Input(format!(
                            "no SQLBuild DAG path exists between {} and {}",
                            python_str_repr(&start_id),
                            python_str_repr(&end_id)
                        ))
                    },
                )?;
            if upstream {
                matched.extend(closure(&BTreeSet::from([start_id]), &index.upstream, true));
            }
            if downstream {
                matched.extend(closure(&BTreeSet::from([end_id]), &index.downstream, true));
            }
            Ok(matched)
        }
        ParsedSelector::Kind {
            kind,
            value,
            upstream,
            downstream,
        } => {
            let mut matched: BTreeSet<String> = match_kind(index, &kind, &value)?;
            let anchors: BTreeSet<String> = matched.clone();
            if upstream {
                matched.extend(closure(&anchors, &index.upstream, true));
            }
            if downstream {
                matched.extend(closure(&anchors, &index.downstream, true));
            }
            Ok(matched)
        }
    }
}

fn match_kind(
    index: &DagSelectionIndex<'_>,
    kind: &str,
    value: &str,
) -> Result<BTreeSet<String>, DagSelectionError> {
    let ids = |matches: &dyn Fn(&DagSelectionNode) -> bool| -> BTreeSet<String> {
        index
            .nodes
            .iter()
            .filter(|(_, node)| matches(node))
            .map(|(id, _)| (*id).to_owned())
            .collect()
    };
    Ok(match kind {
        SELECTOR_KIND_TEST => {
            return Err(DagSelectionError::UnitTest(format!(
                "{SELECTOR_KIND_TEST}{SELECTOR_KIND_SEPARATOR}{value}"
            )));
        }
        SELECTOR_KIND_JOB => {
            return Err(DagSelectionError::Input(format!(
                "selector '{SELECTOR_KIND_JOB}{SELECTOR_KIND_SEPARATOR}{value}' selects a job; \
                 select the job's assets by name, tag or path instead"
            )));
        }
        SELECTOR_KIND_TAG => ids(&|node| node.tags.iter().any(|tag| tag == value)),
        SELECTOR_KIND_PATH => {
            let folder: String = value.replace('\\', "/");
            let folder: &str = folder.trim_matches('/');
            ids(&|node| path_is_under(node.path.as_deref(), folder))
        }
        SELECTOR_KIND_CHECK => index
            .graph
            .checks
            .iter()
            .filter(|check| check.name == value || check.id == value)
            .flat_map(|check| check.checked_asset_ids.iter().cloned())
            .collect(),
        SELECTOR_KIND_NAME => ids(&|node| node.name == value),
        _ => ids(&|node| node.name == value && node.kind == kind),
    })
}

fn path_is_under(path: Option<&str>, folder: &str) -> bool {
    let Some(path) = path else {
        return false;
    };
    let normalized: String = path.replace('\\', "/");
    let normalized: &str = normalized.trim_matches('/');
    let prefixed: String = if !folder.is_empty() && !folder.starts_with(SELECTOR_MODELS_PREFIX) {
        format!("{SELECTOR_MODELS_PREFIX}{folder}")
    } else {
        folder.to_owned()
    };
    [folder, prefixed.as_str()]
        .into_iter()
        .filter(|candidate| !candidate.is_empty())
        .any(|candidate| {
            normalized == candidate
                || normalized
                    .strip_prefix(candidate)
                    .is_some_and(|rest| rest.starts_with('/'))
        })
}

fn node_id_for_name(
    index: &DagSelectionIndex<'_>,
    name: &str,
) -> Result<String, DagSelectionError> {
    let matching: Vec<&str> = index
        .nodes
        .iter()
        .filter(|(_, node)| node.name == name)
        .map(|(id, _)| *id)
        .collect();
    match matching.as_slice() {
        [only] => Ok((*only).to_owned()),
        _ => Err(DagSelectionError::Input(format!(
            "SQLBuild selector name {} matched {} DAG nodes",
            python_str_repr(name),
            matching.len()
        ))),
    }
}
