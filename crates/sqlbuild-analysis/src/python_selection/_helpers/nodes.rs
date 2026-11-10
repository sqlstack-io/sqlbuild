//! Python's `resolve_python_node_selectors`: selector resolution over Python node names.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::Infallible;

use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;
use sqlbuild_core::text::main::glob_match::glob_match;

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;
use crate::graph::main::parse_selector::parse_selector;
use crate::graph::models::{EdgeDirection, ParsedSelector, ProjectGraph};
use crate::python_selection::constants::{
    ASSET_KIND, CHECK_KIND, INTERSECTION_SEPARATOR, JOB_KIND, LOADER_KIND, NAME_KIND,
    PATH_BETWEEN_CODE, PATH_KIND, PATH_ROOT_CODE, PATH_ROOT_ERROR, PATTERN_CHARACTERS,
    SQL_MODEL_PATH_ROOT, TAG_KIND, TASK_KIND, TEST_KIND, UNIT_TEST_CODE,
    UNIT_TEST_ONLY_TEST_AND_BUILD, UNKNOWN_NAME_CODE, UNKNOWN_PATH_CODE, UNKNOWN_TAG_CODE,
    UNMAPPED_KIND_CODE,
};
use crate::python_selection::models::{NodeIndex, PythonSelectionGraph, PythonSelectionNode};
use sqlbuild_core::constants::PYTHON_NODE_ROOT as PYTHON_NODE_PATH_ROOT;

pub(crate) type Names = BTreeSet<String>;
type Resolved = Result<Names, SelectorError>;

pub(crate) fn error(code: &'static str, message: String) -> SelectorError {
    SelectorError {
        code,
        message,
        help: None,
    }
}

/// The error for a unit-test selector where unit tests never run.
pub(crate) fn unit_test_error(raw: &str) -> SelectorError {
    error(
        UNIT_TEST_CODE,
        format!("selector '{raw}' selects a unit test; {UNIT_TEST_ONLY_TEST_AND_BUILD}"),
    )
}

pub(crate) fn is_pattern(value: &str) -> bool {
    value.contains(PATTERN_CHARACTERS)
}

/// A path selector value as a folder: forward slashes, no leading or trailing slash.
pub(crate) fn selector_folder(value: &str) -> String {
    value.replace('\\', "/").trim_matches('/').to_owned()
}

/// The first segment of a selector folder.
pub(crate) fn folder_root(folder: &str) -> &str {
    folder.split('/').next().unwrap_or_default()
}

fn keyed(rows: &[(String, Vec<String>)]) -> HashMap<&str, &[String]> {
    rows.iter()
        .map(|(name, values)| (name.as_str(), values.as_slice()))
        .collect()
}

impl<'graph> NodeIndex<'graph> {
    pub(crate) fn new(graph: &'graph PythonSelectionGraph) -> Self {
        Self {
            graph,
            by_name: graph
                .nodes
                .iter()
                .map(|node| (node.name.as_str(), node))
                .collect(),
            upstream: keyed(&graph.upstream),
            downstream: keyed(&graph.downstream),
            tags: keyed(&graph.tags),
        }
    }

    pub(crate) fn node(&self, name: &str) -> Option<&'graph PythonSelectionNode> {
        self.by_name.get(name).copied()
    }

    pub(crate) fn edges(&self, name: &str, direction: EdgeDirection) -> &'graph [String] {
        let edges: &HashMap<&'graph str, &'graph [String]> = match direction {
            EdgeDirection::Upstream => &self.upstream,
            EdgeDirection::Downstream => &self.downstream,
        };
        edges.get(name).copied().unwrap_or_default()
    }

    /// Every name reachable from `starts`; a start itself only through a cycle.
    pub(crate) fn closure<'name>(
        &self,
        starts: impl IntoIterator<Item = &'name str>,
        direction: EdgeDirection,
    ) -> Names {
        let mut visited: HashSet<String> = HashSet::new();
        let Ok(()) = breadth_first_reach(
            starts.into_iter().map(str::to_owned),
            None,
            &mut visited,
            |current| Ok::<_, Infallible>(self.edges(current, direction).iter().cloned()),
        );
        visited.into_iter().collect()
    }

    /// Whether `name` is a node of `kind`.
    pub(crate) fn is_kind(&self, name: &str, kind: &str) -> bool {
        self.node(name).is_some_and(|node| node.kind == kind)
    }
}

/// `select` minus `exclude` over Python nodes; every node when nothing is selected.
pub(crate) fn select_nodes(
    index: &NodeIndex<'_>,
    select: &[String],
    exclude: &[String],
) -> Resolved {
    if select.is_empty() {
        return Ok(index
            .by_name
            .keys()
            .map(|name| (*name).to_owned())
            .collect());
    }
    let selected: Names = groups(index, select)?;
    let excluded: Names = groups(index, exclude)?;
    Ok(selected.difference(&excluded).cloned().collect())
}

fn groups(index: &NodeIndex<'_>, raw_groups: &[String]) -> Resolved {
    let mut resolved: Names = Names::new();
    for group in raw_groups {
        for token in group.split_whitespace() {
            resolved.extend(intersection(index, token)?);
        }
    }
    Ok(resolved)
}

fn intersection(index: &NodeIndex<'_>, token: &str) -> Resolved {
    let parts: Vec<Names> = token
        .split(INTERSECTION_SEPARATOR)
        .map(|part| single(index, part))
        .collect::<Result<_, _>>()?;
    let mut parts = parts.into_iter();
    let mut result: Names = parts.next().unwrap_or_default();
    for part in parts {
        result = result.intersection(&part).cloned().collect();
    }
    Ok(result)
}

fn single(index: &NodeIndex<'_>, raw: &str) -> Resolved {
    let ParsedSelector::Kind {
        kind,
        value,
        upstream,
        downstream,
    } = parse_selector(raw)?
    else {
        return Err(error(
            PATH_BETWEEN_CODE,
            format!("path-between selector '{raw}' is not supported for Python nodes yet"),
        ));
    };
    let names: Names = match kind.as_str() {
        TEST_KIND => return Err(unit_test_error(raw)),
        TAG_KIND => tagged(index, &value)?,
        PATH_KIND => under_path(index, &value)?,
        _ => {
            let names: Names = lookup(index, &kind, &value)?;
            if names.is_empty() {
                return Err(error(
                    UNKNOWN_NAME_CODE,
                    format!("unknown Python node selector '{value}'"),
                ));
            }
            names
        }
    };
    Ok(expanded(index, names, (upstream, downstream)))
}

fn tagged(index: &NodeIndex<'_>, tag: &str) -> Resolved {
    let names: Names = index
        .tags
        .get(tag)
        .copied()
        .unwrap_or_default()
        .iter()
        .cloned()
        .collect();
    if names.is_empty() {
        return Err(error(
            UNKNOWN_TAG_CODE,
            format!("no Python nodes found with tag '{tag}'"),
        ));
    }
    Ok(names)
}

fn under_path(index: &NodeIndex<'_>, value: &str) -> Resolved {
    let folder: String = selector_folder(value);
    let root: &str = folder_root(&folder);
    if root != PYTHON_NODE_PATH_ROOT && root != SQL_MODEL_PATH_ROOT {
        return Err(error(PATH_ROOT_CODE, PATH_ROOT_ERROR.to_owned()));
    }
    let nested: String = format!("{folder}/");
    let names: Names = index
        .graph
        .paths
        .iter()
        .filter(|(_, indexed)| {
            folder.is_empty() || *indexed == folder || indexed.starts_with(&nested)
        })
        .map(|(name, _)| name.clone())
        .collect();
    if names.is_empty() {
        return Err(error(
            UNKNOWN_PATH_CODE,
            format!("no Python nodes found under path '{folder}'"),
        ));
    }
    Ok(names)
}

fn lookup(index: &NodeIndex<'_>, kind: &str, value: &str) -> Resolved {
    if kind == NAME_KIND {
        if !is_pattern(value) {
            return Ok(index
                .node(value)
                .map(|node| node.name.clone())
                .into_iter()
                .collect());
        }
        return Ok(index
            .by_name
            .iter()
            .filter(|(name, _)| glob_match(name, value))
            .map(|(_, node)| node.name.clone())
            .collect());
    }
    if ![TASK_KIND, ASSET_KIND, LOADER_KIND, CHECK_KIND].contains(&kind) {
        return Err(error(
            UNMAPPED_KIND_CODE,
            format!("selector type '{kind}' does not map to a Python node type"),
        ));
    }
    if !is_pattern(value) {
        return Ok(index
            .graph
            .nodes
            .iter()
            .rev()
            .find(|node| node.kind == kind && node.name == value)
            .map(|node| node.name.clone())
            .into_iter()
            .collect());
    }
    Ok(index
        .by_name
        .values()
        .filter(|node| node.kind == kind && glob_match(&node.name, value))
        .map(|node| node.name.clone())
        .collect())
}

fn expanded(index: &NodeIndex<'_>, names: Names, (upstream, downstream): (bool, bool)) -> Names {
    let mut result: Names = names.clone();
    if upstream {
        result.extend(index.closure(names.iter().map(String::as_str), EdgeDirection::Upstream));
    }
    if downstream {
        result.extend(index.closure(names.iter().map(String::as_str), EdgeDirection::Downstream));
    }
    result
}

/// Python's `all_keys`: selector name to SQL key.
pub(crate) fn sql_names(graph: &ProjectGraph) -> HashMap<&str, &ObjectKey> {
    graph
        .names
        .iter()
        .map(|(name, key)| (name.as_str(), key))
        .collect()
}

/// Whether a part of `token` is a `job:` selector; jobs select only SQL resources.
pub(crate) fn names_job(token: &str) -> bool {
    token.split(INTERSECTION_SEPARATOR).any(|part| {
        matches!(parse_selector(part), Ok(ParsedSelector::Kind { kind, .. }) if kind == JOB_KIND)
    })
}
