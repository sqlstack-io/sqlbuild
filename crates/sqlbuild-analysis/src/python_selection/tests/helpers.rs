use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;
use crate::graph::main::build_project_graph::build_project_graph;
use crate::graph::models::{GraphResource, ProjectGraph};
use crate::python_selection::main::published_python_graph::published_python_graph;
use crate::python_selection::main::python_node_graph_indexes::python_node_graph_indexes;
use crate::python_selection::main::resolve_python_node_excludes::resolve_python_node_excludes;
use crate::python_selection::main::resolve_python_node_selectors::resolve_python_node_selectors;
use crate::python_selection::main::resolve_python_sql_run_selection::resolve_python_sql_run_selection;
use crate::python_selection::models::{
    InventoryIndexes, InventoryNode, PublishedPythonNode, PythonSelectionGraph,
    PythonSelectionNode, RunSelection, SqlSelectionGraph,
};
use crate::python_selection::tests::test_types::{ExpectedRun, SelectorFailure};

type OwnedFailure = (String, String);
type OwnedRun = (Vec<ObjectKey>, Vec<String>);

/// `(name, kind, relative path, tags, upstream nodes, sql deps)`.
type NodeSpec = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    &'static [(&'static str, &'static str)],
);

const ORDERS_NODES: &[NodeSpec] = &[
    (
        "load_events",
        "loader",
        "python/loaders/events.py",
        &[],
        &["prepare_orders"],
        &[],
    ),
    (
        "prepare_orders",
        "task",
        "python/tasks/orders.py",
        &["orders", "daily"],
        &[],
        &[],
    ),
    (
        "export_orders",
        "asset",
        "python/assets/orders.py",
        &["exports", "daily"],
        &["prepare_orders"],
        &[("model", "orders")],
    ),
    (
        "check_orders_export",
        "check",
        "python/checks/orders.py",
        &["exports"],
        &["export_orders"],
        &[],
    ),
    (
        "raw_orders",
        "loader",
        "python/loaders/orders.py",
        &[],
        &[],
        &[],
    ),
];

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn key(pair: &(&str, &str)) -> ObjectKey {
    (pair.0.to_owned(), pair.1.to_owned())
}

fn failure(error: SelectorError) -> OwnedFailure {
    (error.code.to_owned(), error.message)
}

/// The inventory rows of the orders Python nodes.
fn inventory_nodes() -> Vec<InventoryNode> {
    ORDERS_NODES
        .iter()
        .map(|(name, _, path, tags, upstream, _)| InventoryNode {
            name: (*name).to_owned(),
            tags: owned(tags),
            relative_path: (*path).to_owned(),
            upstream: owned(upstream),
        })
        .collect()
}

/// The orders Python node graph, indexed by the native inventory.
pub(super) fn orders_python_graph() -> PythonSelectionGraph {
    let indexes: InventoryIndexes = python_node_graph_indexes(&inventory_nodes());
    PythonSelectionGraph {
        nodes: ORDERS_NODES
            .iter()
            .map(|(name, kind, _, _, _, sql_deps)| PythonSelectionNode {
                name: (*name).to_owned(),
                kind: (*kind).to_owned(),
                sql_deps: sql_deps.iter().map(key).collect(),
            })
            .collect(),
        upstream: indexes.upstream,
        downstream: indexes.downstream,
        tags: indexes.tags,
        paths: indexes.paths,
    }
}

fn orders_sql_graph() -> ProjectGraph {
    let raw_orders: ObjectKey = key(&("source", "raw_orders"));
    build_project_graph(&[
        GraphResource {
            key: key(&("model", "orders")),
            deps: vec![raw_orders.clone()],
            tags: owned(&["daily"]),
            folder: Some(String::new()),
        },
        GraphResource {
            key: raw_orders,
            deps: Vec::new(),
            tags: Vec::new(),
            folder: None,
        },
    ])
}

/// The orders Python node graph as the server rebuilds it from published snapshot nodes.
pub(super) fn published_orders_graph() -> PythonSelectionGraph {
    let nodes: Vec<PublishedPythonNode> = ORDERS_NODES
        .iter()
        .map(
            |(name, kind, path, tags, upstream, sql_deps)| PublishedPythonNode {
                name: (*name).to_owned(),
                kind: (*kind).to_owned(),
                tags: owned(tags),
                folder: path
                    .rsplit_once('/')
                    .map_or(".", |(folder, _)| folder)
                    .to_owned(),
                deps: upstream
                    .iter()
                    .map(|upstream| {
                        let kind: &str = ORDERS_NODES
                            .iter()
                            .find(|node| node.0 == *upstream)
                            .map_or("task", |node| node.1);
                        key(&(kind, upstream))
                    })
                    .chain(sql_deps.iter().map(key))
                    .collect(),
            },
        )
        .collect();
    published_python_graph(&nodes)
}

/// The Python node names the selectors resolve to over the orders graph.
pub(super) fn node_names(select: &[&str], exclude: &[&str]) -> Result<Vec<String>, OwnedFailure> {
    resolve_python_node_selectors(&orders_python_graph(), &owned(select), &owned(exclude))
        .map_err(failure)
}

/// The run selection over the orders SQL and Python graphs.
pub(super) fn run(select: &[&str], exclude: &[&str]) -> Result<OwnedRun, OwnedFailure> {
    run_over(&orders_python_graph(), select, exclude)
}

/// The run selection over the orders SQL graph and the given Python graph.
pub(super) fn run_over(
    python: &PythonSelectionGraph,
    select: &[&str],
    exclude: &[&str],
) -> Result<OwnedRun, OwnedFailure> {
    let graph: ProjectGraph = orders_sql_graph();
    let sources: Vec<(String, Option<String>)> =
        vec![("raw_orders".to_owned(), Some("raw_orders".to_owned()))];
    resolve_python_sql_run_selection(
        SqlSelectionGraph {
            graph: &graph,
            sources: &sources,
        },
        python,
        &owned(select),
        &owned(exclude),
    )
    .map(
        |RunSelection {
             sql_keys,
             python_node_names,
         }| (sql_keys, python_node_names),
    )
    .map_err(failure)
}

/// An owned copy of an expected node-name result.
pub(super) fn expected_names(
    expected: Result<&[&str], SelectorFailure>,
) -> Result<Vec<String>, OwnedFailure> {
    expected
        .map(owned)
        .map_err(|(code, message)| (code.to_owned(), message.to_owned()))
}

/// An owned copy of an expected run result.
pub(super) fn expected_run(
    expected: Result<ExpectedRun, SelectorFailure>,
) -> Result<OwnedRun, OwnedFailure> {
    expected
        .map(|(keys, names)| (keys.iter().map(key).collect(), owned(names)))
        .map_err(|(code, message)| (code.to_owned(), message.to_owned()))
}

/// The folder the inventory indexes for one relative path.
pub(super) fn indexed_folder(relative_path: &str) -> String {
    let nodes: Vec<InventoryNode> = vec![InventoryNode {
        name: "node".to_owned(),
        tags: Vec::new(),
        relative_path: relative_path.to_owned(),
        upstream: Vec::new(),
    }];
    python_node_graph_indexes(&nodes)
        .paths
        .into_iter()
        .map(|(_, folder)| folder)
        .collect()
}

/// The Python node names `sqb check` exclude tokens name over the orders graph.
pub(super) fn excluded_names(exclude: &[&str]) -> Result<Vec<String>, OwnedFailure> {
    resolve_python_node_excludes(&orders_python_graph(), &owned(exclude)).map_err(failure)
}
