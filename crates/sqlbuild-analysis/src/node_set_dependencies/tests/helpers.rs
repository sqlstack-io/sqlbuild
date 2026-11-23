use std::collections::{BTreeMap, HashMap};

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;
use crate::graph::main::build_project_graph::build_project_graph;
use crate::graph::models::{GraphResource, JobSelection, ProjectGraph};
use crate::node_set_dependencies::_helpers::labels::node_label;
use crate::node_set_dependencies::main::resolve_node_set_dependencies::resolve_node_set_dependencies;
use crate::node_set_dependencies::main::set_dependency_metadata::set_dependency_metadata;
use crate::node_set_dependencies::models::{DependantNode, NodeSetDependency, ResolvedNode};
use crate::python_selection::main::python_node_graph_indexes::python_node_graph_indexes;
use crate::python_selection::models::{
    InventoryIndexes, InventoryNode, PythonSelectionGraph, PythonSelectionNode, SqlSelectionGraph,
};

/// `(name, kind, tags, upstream nodes, sql deps)`.
type NodeSpec = (
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    &'static [(&'static str, &'static str)],
);
/// `(kind, name, exclude)` of one set dependency.
pub(super) type SetSpec = (&'static str, &'static str, Option<&'static str>);

const NODES: &[NodeSpec] = &[
    ("report", "asset", &["reports"], &[], &[]),
    ("export_orders", "asset", &[], &[], &[("model", "orders")]),
    (
        "audit_export",
        "asset",
        &[],
        &["export_orders", "report"],
        &[],
    ),
    ("self_tagged", "asset", &["finance"], &[], &[]),
    ("sync_events", "loader", &[], &[], &[]),
];

/// `(resource type, name, tags, dependencies)` of each SQL resource.
const RESOURCES: &[(&str, &str, &[&str], &[&str])] = &[
    ("source", "raw_orders", &[], &[]),
    ("model", "stg_orders", &[], &["source:raw_orders"]),
    ("model", "orders", &["finance"], &["model:stg_orders"]),
    ("model", "order_items", &["finance"], &["model:stg_orders"]),
    (
        "model",
        "finance_daily",
        &[],
        &["model:orders", "model:order_items"],
    ),
    ("model", "customers", &["crm"], &[]),
    (
        "model",
        "customer_orders",
        &[],
        &["model:customers", "model:orders"],
    ),
];

/// Each job's `select`; a group lists its member jobs.
const JOBS: &[(&str, &str)] = &[
    ("nightly", "stg_orders orders order_items"),
    ("single", "customers"),
    ("diamond", "stg_orders orders order_items finance_daily"),
    ("with_asset", "orders asset:export_orders"),
    ("loop", "asset:export_orders asset:audit_export"),
    ("nightly_wide", "orders customer_orders"),
    ("reporting", "asset:report customers"),
];

fn key(spec: &str) -> ObjectKey {
    let (kind, name) = spec.split_once(':').unwrap_or(("model", spec));
    (kind.to_owned(), name.to_owned())
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn sql_graph() -> ProjectGraph {
    build_project_graph(
        &RESOURCES
            .iter()
            .map(|(kind, name, tags, deps)| GraphResource {
                key: ((*kind).to_owned(), (*name).to_owned()),
                deps: deps.iter().map(|dep| key(dep)).collect(),
                tags: owned(tags),
                folder: (*kind == "model").then(String::new),
            })
            .collect::<Vec<_>>(),
    )
}

fn python_graph() -> PythonSelectionGraph {
    let inventory: Vec<InventoryNode> = NODES
        .iter()
        .map(|(name, _, tags, upstream, _)| InventoryNode {
            name: (*name).to_owned(),
            tags: owned(tags),
            relative_path: format!("python/nodes/{name}.py"),
            upstream: owned(upstream),
        })
        .collect();
    let indexes: InventoryIndexes = python_node_graph_indexes(&inventory);
    PythonSelectionGraph {
        nodes: NODES
            .iter()
            .map(|(name, kind, _, _, sql_deps)| PythonSelectionNode {
                name: (*name).to_owned(),
                kind: (*kind).to_owned(),
                sql_deps: sql_deps
                    .iter()
                    .map(|(kind, name)| ((*kind).to_owned(), (*name).to_owned()))
                    .collect(),
            })
            .collect(),
        upstream: indexes.upstream,
        downstream: indexes.downstream,
        tags: indexes.tags,
        paths: indexes.paths,
    }
}

/// The fixture's jobs and the job groups `groups` lists.
fn catalog(groups: &[(&str, &[&str])]) -> BTreeMap<String, JobSelection> {
    let mut catalog: BTreeMap<String, JobSelection> = JOBS
        .iter()
        .map(|(name, select)| {
            (
                (*name).to_owned(),
                JobSelection::Job {
                    select: select.split_whitespace().map(str::to_owned).collect(),
                    exclude: Vec::new(),
                },
            )
        })
        .collect();
    catalog.extend(groups.iter().map(|(name, members)| {
        (
            (*name).to_owned(),
            JobSelection::Group {
                members: owned(members),
            },
        )
    }));
    catalog
}

/// The sets of `dependant` resolved over the fixture project.
pub(super) fn resolve(
    (dependant, kind): (&str, &str),
    sets: &[SetSpec],
    groups: &[(&str, &[&str])],
) -> Result<Vec<ResolvedNode>, SelectorError> {
    let graph: ProjectGraph = sql_graph();
    let sources: Vec<(String, Option<String>)> = vec![("raw_orders".to_owned(), None)];
    resolve_node_set_dependencies(
        SqlSelectionGraph {
            graph: &graph,
            sources: &sources,
            allow_warehouse_round_trips: false,
        },
        &python_graph(),
        &[DependantNode {
            name: dependant.to_owned(),
            kind: kind.to_owned(),
            sets: sets
                .iter()
                .map(|(kind, name, exclude)| NodeSetDependency {
                    kind: (*kind).to_owned(),
                    name: (*name).to_owned(),
                    exclude: exclude.map(str::to_owned),
                })
                .collect(),
        }],
        &catalog(groups),
    )
}

/// Each resolved set's end nodes, labelled as errors name them.
pub(super) fn ends(resolved: &[ResolvedNode]) -> Vec<Vec<String>> {
    let kinds: HashMap<String, String> = NODES
        .iter()
        .map(|spec| (spec.0.to_owned(), spec.1.to_owned()))
        .collect();
    resolved
        .iter()
        .flat_map(|node| &node.sets)
        .map(|set| set.ends.iter().map(|end| node_label(end, &kinds)).collect())
        .collect()
}

/// The fragments `text` lacks.
pub(super) fn missing<'a>(text: &str, fragments: &[&'a str]) -> Vec<&'a str> {
    fragments
        .iter()
        .filter(|fragment| !text.contains(**fragment))
        .copied()
        .collect()
}

/// Base identity metadata with `sets` recorded as `(set label, end node labels)`.
pub(super) fn recorded(base: &str, sets: &[(&str, &[&str])]) -> String {
    let owned_sets: Vec<(String, Vec<String>)> = sets
        .iter()
        .map(|(set, ends)| ((*set).to_owned(), owned(ends)))
        .collect();
    set_dependency_metadata(base, &owned_sets)
}
