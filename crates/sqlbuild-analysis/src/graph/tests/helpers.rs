use std::collections::{BTreeMap, BTreeSet};

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;
use crate::graph::main::attach_jobs::attach_jobs;
use crate::graph::main::build_project_graph::build_project_graph;
use crate::graph::main::resolve_selectors::resolve_selectors;
use crate::graph::models::{GraphResource, JobSelection, ParsedSelector, ProjectGraph};
use crate::graph::tests::test_types::SelectorFailure;

type OwnedFailure = (String, String, Option<String>);

pub(super) type ResourceSpec = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
    &'static [&'static str],
    Option<&'static str>,
);

/// Models, a source, a seed and a UDF with tags and folders, in Python's project order.
const ORDERS_PROJECT: &[ResourceSpec] = &[
    (
        "model",
        "orders",
        &[("source", "raw_orders"), ("udf", "tax")],
        &["daily"],
        Some("mart"),
    ),
    (
        "model",
        "order_items",
        &[("model", "orders"), ("seed", "countries")],
        &["daily", "finance"],
        Some("models/mart/items"),
    ),
    ("model", "customers", &[], &[], Some("staging/crm")),
    (
        "model",
        "customer_orders",
        &[("model", "customers"), ("model", "orders")],
        &[],
        Some("models"),
    ),
    ("source", "raw_orders", &[], &[], None),
    ("seed", "countries", &[], &["ref"], None),
    ("udf", "tax", &[], &[], None),
];

pub(super) fn key(pair: &(&str, &str)) -> ObjectKey {
    (pair.0.to_owned(), pair.1.to_owned())
}

/// Two sources feeding two models that a third joins, plus a seed nothing reads.
pub(super) const DIAMOND_PROJECT: &[ResourceSpec] = &[
    (
        "model",
        "orders",
        &[("source", "raw_orders")],
        &["nightly", "staging"],
        None,
    ),
    (
        "model",
        "customers",
        &[("source", "raw_customers")],
        &[],
        None,
    ),
    (
        "model",
        "joined",
        &[("model", "orders"), ("model", "customers")],
        &["nightly"],
        None,
    ),
    ("source", "raw_orders", &[], &[], None),
    ("source", "raw_customers", &[], &[], None),
    ("seed", "codes", &[], &[], None),
];

/// Staging, nested staging, intermediate and mart folders over two sources.
pub(super) const FOLDER_PROJECT: &[ResourceSpec] = &[
    (
        "model",
        "stg_orders",
        &[("source", "raw_orders")],
        &[],
        Some("staging"),
    ),
    (
        "model",
        "stg_customers",
        &[("source", "raw_customers")],
        &[],
        Some("staging"),
    ),
    (
        "model",
        "stg_deep",
        &[("source", "raw_orders")],
        &[],
        Some("staging/raw"),
    ),
    (
        "model",
        "int_enriched",
        &[("model", "stg_orders"), ("model", "stg_customers")],
        &[],
        Some("intermediate"),
    ),
    (
        "model",
        "fact_orders",
        &[("model", "int_enriched")],
        &[],
        Some("marts"),
    ),
    ("source", "raw_orders", &[], &[], None),
    ("source", "raw_customers", &[], &[], None),
];

/// A model read through a table function by another model.
pub(super) const TABLE_FUNCTION_PROJECT: &[ResourceSpec] = &[
    ("model", "orders", &[], &[], None),
    (
        "table_fn",
        "customer_orders",
        &[("model", "orders")],
        &[],
        None,
    ),
    (
        "model",
        "customer_order_summary",
        &[("table_fn", "customer_orders")],
        &[],
        None,
    ),
];

pub(super) fn orders_graph() -> ProjectGraph {
    project_graph(ORDERS_PROJECT)
}

/// The graph of a resource table.
pub(super) fn project_graph(project: &[ResourceSpec]) -> ProjectGraph {
    let resources: Vec<GraphResource> = project
        .iter()
        .map(|(kind, name, deps, tags, folder)| GraphResource {
            key: key(&(kind, name)),
            deps: deps.iter().map(key).collect(),
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            folder: folder.map(str::to_owned),
        })
        .collect();
    build_project_graph(&resources)
}

/// The resolved keys, or the error's `(code, message, help)`.
pub(super) fn resolved(select: &[&str], exclude: &[&str]) -> Result<Vec<ObjectKey>, OwnedFailure> {
    resolved_in(&orders_graph(), select, exclude)
}

/// The names `select` minus `exclude` resolves to in a project, sorted, or the error.
pub(super) fn resolved_names(
    project: &[ResourceSpec],
    select: &[&str],
    exclude: &[&str],
) -> Result<BTreeSet<String>, OwnedFailure> {
    resolved_in(&project_graph(project), select, exclude)
        .map(|keys| keys.into_iter().map(|(_, name)| name).collect())
}

fn resolved_in(
    graph: &ProjectGraph,
    select: &[&str],
    exclude: &[&str],
) -> Result<Vec<ObjectKey>, OwnedFailure> {
    let owned = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    };
    resolve_selectors(graph, &owned(select), &owned(exclude)).map_err(
        |SelectorError {
             code,
             message,
             help,
         }| (code.to_owned(), message, help),
    )
}

/// The keys a selector case expects.
pub(super) fn keys(pairs: &[(&str, &str)]) -> Vec<ObjectKey> {
    pairs.iter().map(key).collect()
}

/// An owned copy of the `(code, message, help)` a selector case expects.
pub(super) fn failure((code, message, help): SelectorFailure) -> OwnedFailure {
    (code.to_owned(), message.to_owned(), help.map(str::to_owned))
}

/// A sorted name set.
pub(super) fn names(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// A `kind:value` selector with its expansion flags.
pub(super) fn kind(kind: &str, value: &str, upstream: bool, downstream: bool) -> ParsedSelector {
    ParsedSelector::Kind {
        kind: kind.to_owned(),
        value: value.to_owned(),
        upstream,
        downstream,
    }
}

/// A `start~end` selector with its expansion flags.
pub(super) fn path(start: &str, end: &str, upstream: bool, downstream: bool) -> ParsedSelector {
    ParsedSelector::Path {
        start: start.to_owned(),
        end: end.to_owned(),
        upstream,
        downstream,
    }
}

/// Jobs over the orders project: valid jobs, a group, and jobs whose own selection fails.
fn orders_jobs() -> BTreeMap<String, JobSelection> {
    let job = |select: &[&str], exclude: &[&str]| JobSelection::Job {
        select: select.iter().map(|value| (*value).to_owned()).collect(),
        exclude: exclude.iter().map(|value| (*value).to_owned()).collect(),
    };
    let group = |members: &[&str]| JobSelection::Group {
        members: members.iter().map(|value| (*value).to_owned()).collect(),
    };
    BTreeMap::from([
        ("nightly".to_owned(), job(&["orders"], &[])),
        ("crm".to_owned(), job(&["customers"], &[])),
        ("marts".to_owned(), job(&["orders+"], &[])),
        (
            "marts_without_items".to_owned(),
            job(&["orders+"], &["order_items"]),
        ),
        ("daily_group".to_owned(), group(&["nightly", "crm"])),
        ("broken".to_owned(), job(&["missing"], &[])),
        ("nested".to_owned(), job(&["job:nightly"], &[])),
        ("ghost_group".to_owned(), group(&["nightly", "ghost"])),
    ])
}

/// The names `select` minus `exclude` resolves to in the orders project, with or without its jobs.
pub(super) fn job_selection(
    attached: bool,
    select: &[&str],
    exclude: &[&str],
) -> Result<BTreeSet<String>, OwnedFailure> {
    let mut graph: ProjectGraph = attach_jobs(
        orders_graph(),
        orders_jobs().into_iter().filter(|_| attached).collect(),
    );
    graph.jobs = graph.jobs.filter(|_| attached);
    resolved_in(&graph, select, exclude)
        .map(|keys| keys.into_iter().map(|(_, name)| name).collect())
}
