//! Python's SQL/Python graph boundary rules and selected-dependency checks.

use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};

use sqlbuild_core::graph_walk::main::find_cycle::find_cycle as find_dependency_cycle;

use crate::assembly::project::types::ObjectKey;

use crate::graph::errors::SelectorError;
use crate::graph::models::EdgeDirection;
use crate::python_selection::_helpers::nodes::error;
use crate::python_selection::constants::{
    ASSET_KIND, CHECK_KIND, LOADER_KIND, MODEL_RESOURCE, PLANNER_DEFAULT_CODE, SOURCE_RESOURCE,
    SQL_MODEL_DEPENDENCY_TYPES,
};
use crate::python_selection::models::{Atom, SelectionContext, SqlSelectionGraph};

fn failure(message: String) -> SelectorError {
    error(PLANNER_DEFAULT_CODE, message)
}

/// `{source name: loader}` for sources with a loader, as Python's dict builds it.
pub(crate) fn source_loaders<'graph>(
    sql: SqlSelectionGraph<'graph>,
) -> Vec<(&'graph str, &'graph str)> {
    let mut rows: Vec<(&str, &str)> = Vec::new();
    let mut positions: HashMap<&str, usize> = HashMap::new();
    for (source, loader) in sql.sources {
        let Some(loader) = loader else {
            continue;
        };
        match positions.get(source.as_str()) {
            Some(&position) => rows[position].1 = loader,
            None => {
                positions.insert(source, rows.len());
                rows.push((source, loader));
            }
        }
    }
    rows
}

/// `{loader: source}` over [`source_loaders`], a later source winning a shared loader.
pub(crate) fn terminal_loaders<'graph>(
    sql: SqlSelectionGraph<'graph>,
) -> HashMap<&'graph str, &'graph str> {
    source_loaders(sql)
        .into_iter()
        .map(|(source, loader)| (loader, source))
        .collect()
}

/// Python's `validate_python_sql_boundaries`.
pub(crate) fn validate_boundaries(context: &SelectionContext<'_>) -> Result<(), SelectorError> {
    validate_sql_model_dependencies(context)?;
    validate_python_sql_refs(context)?;
    validate_loader_upstreams(context)?;
    let terminal: HashMap<&str, &str> = terminal_loaders(context.sql);
    for node in &context.nodes.graph.nodes {
        for dependency in context.nodes.edges(&node.name, EdgeDirection::Upstream) {
            let Some(source) = terminal.get(dependency.as_str()) else {
                continue;
            };
            if node.kind == CHECK_KIND {
                return Err(failure(format!(
                    "Check '{}' depends on terminal loader '{dependency}'; \
                     use source audits for source '{source}' instead",
                    node.name
                )));
            }
            if node.kind == ASSET_KIND {
                return Err(failure(format!(
                    "Python node '{}' depends on terminal loader '{dependency}'; \
                     depend on source '{source}' instead",
                    node.name
                )));
            }
        }
    }
    Ok(())
}

fn validate_sql_model_dependencies(context: &SelectionContext<'_>) -> Result<(), SelectorError> {
    let sources: HashSet<&str> = context
        .sql
        .sources
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    for (model, dependencies) in &context.sql.graph.upstream {
        if model.0 != MODEL_RESOURCE {
            continue;
        }
        for dependency in dependencies {
            if !SQL_MODEL_DEPENDENCY_TYPES.contains(&dependency.0.as_str()) {
                return Err(failure(format!(
                    "SQL model '{}' depends on non-SQL resource '{}:{}'; \
                     SQL model dependencies must stay SQL-only",
                    model.1, dependency.0, dependency.1
                )));
            }
            if dependency.0 == SOURCE_RESOURCE && !sources.contains(dependency.1.as_str()) {
                return Err(failure(format!(
                    "SQL model '{}' depends on intermediate loader '{}'; \
                     depend on a source populated by a terminal loader instead",
                    model.1, dependency.1
                )));
            }
        }
    }
    Ok(())
}

fn validate_python_sql_refs(context: &SelectionContext<'_>) -> Result<(), SelectorError> {
    for node in &context.nodes.graph.nodes {
        for (kind, name) in &node.sql_deps {
            let Some(key) = context.sql_names.get(name.as_str()) else {
                return Err(failure(format!(
                    "Python node '{}' depends on unknown SQL resource '{name}'",
                    node.name
                )));
            };
            if &key.0 != kind {
                return Err(failure(format!(
                    "Python node '{}' declares {kind}('{name}') but '{name}' is a {}",
                    node.name, key.0
                )));
            }
        }
    }
    Ok(())
}

/// The dependency cycle through a loader, which every cycle across SQL and Python passes.
fn dependency_cycle(context: &SelectionContext<'_>) -> Option<SelectorError> {
    let cycle: Vec<String> = round_trip_cycle(context, &loader_names(context))?;
    Some(SelectorError {
        code: PLANNER_DEFAULT_CODE,
        message: format!(
            "{} is in a dependency cycle: {}",
            cycle[0],
            cycle.join(" -> ")
        ),
        help: Some(
            "each arrow points to what a node depends on; an asset upstream of a \
             loader cannot read anything built from the source that loader fills"
                .to_owned(),
        ),
    })
}

fn loader_names<'graph>(context: &SelectionContext<'graph>) -> Vec<&'graph str> {
    context
        .nodes
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == LOADER_KIND)
        .map(|node| node.name.as_str())
        .collect()
}

fn validate_loader_upstreams(context: &SelectionContext<'_>) -> Result<(), SelectorError> {
    if let Some(cycle) = dependency_cycle(context) {
        return Err(cycle);
    }
    let loaders: Vec<&str> = loader_names(context);
    if context.sql.allow_warehouse_round_trips {
        return Ok(());
    }
    let terminal: HashMap<&str, &str> = terminal_loaders(context.sql);
    for loader in loaders {
        let Some(path) = sql_reading_upstream(context, loader) else {
            continue;
        };
        let reader: &str = path[path.len() - 1];
        let kind: &str = context
            .nodes
            .node(reader)
            .map_or(ASSET_KIND, |node| &node.kind);
        let through: String = if path.len() > 1 {
            let names: Vec<String> = path[..path.len() - 1]
                .iter()
                .map(|name| python_label(context, name))
                .collect();
            format!(" (through {})", names.join(", "))
        } else {
            String::new()
        };
        let source: &str = terminal.get(loader).copied().unwrap_or(loader);
        return Err(SelectorError {
            code: PLANNER_DEFAULT_CODE,
            message: format!(
                "loader '{loader}' depends on {kind} '{reader}'{through}, which reads SQL\n  \
                 This is a warehouse round trip: data leaves the warehouse and comes back \
                 through this loader.\n  Lineage and column lineage restart at the source \
                 '{source}', and builds and job\n  groups run the {kind} before the loader."
            ),
            help: Some(
                "if this is intended, set `allow_warehouse_round_trips = true` under [settings] \
                 in sqlbuild_project.toml"
                    .to_owned(),
            ),
        });
    }
    Ok(())
}

/// `kind 'name'` of a Python node, as round-trip errors name it.
fn python_label(context: &SelectionContext<'_>, name: &str) -> String {
    let kind: &str = context.nodes.node(name).map_or("node", |node| &node.kind);
    format!("{kind} '{name}'")
}

/// The nearest asset upstream of `loader` that reads SQL, with the Python nodes between.
fn sql_reading_upstream<'graph>(
    context: &SelectionContext<'graph>,
    loader: &str,
) -> Option<Vec<&'graph str>> {
    let mut parents: HashMap<&'graph str, Option<&'graph str>> = HashMap::new();
    let mut queue: VecDeque<&'graph str> = VecDeque::new();
    for upstream in context.nodes.edges(loader, EdgeDirection::Upstream) {
        if !parents.contains_key(upstream.as_str()) {
            parents.insert(upstream, None);
            queue.push_back(upstream);
        }
    }
    while let Some(name) = queue.pop_front() {
        if let Some(node) = context.nodes.node(name)
            && node.kind == ASSET_KIND
            && !node.sql_deps.is_empty()
        {
            let mut path: Vec<&'graph str> = vec![name];
            let mut current: Option<&'graph str> = parents[name];
            while let Some(parent) = current {
                path.push(parent);
                current = parents[parent];
            }
            path.reverse();
            return Some(path);
        }
        for upstream in context.nodes.edges(name, EdgeDirection::Upstream) {
            if upstream != loader && !parents.contains_key(upstream.as_str()) {
                parents.insert(upstream, Some(name));
                queue.push_back(upstream);
            }
        }
    }
    None
}

/// A node of the combined dependency graph a round trip can close a cycle in.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum RoundTripNode {
    Python(String),
    Sql(ObjectKey),
}

/// The first cycle through a loader, its Python upstreams, the SQL they read and the source it fills.
fn round_trip_cycle(context: &SelectionContext<'_>, loaders: &[&str]) -> Option<Vec<String>> {
    let source_loaders: HashMap<&str, &str> = source_loaders(context.sql).into_iter().collect();
    let roots = loaders
        .iter()
        .map(|name| RoundTripNode::Python((*name).to_owned()));
    let path: Vec<RoundTripNode> = find_dependency_cycle(roots, |node| match node {
        RoundTripNode::Python(name) => {
            let mut next: Vec<RoundTripNode> = context
                .nodes
                .edges(name, EdgeDirection::Upstream)
                .iter()
                .map(|upstream| RoundTripNode::Python(upstream.clone()))
                .collect();
            if let Some(node) = context.nodes.node(name)
                && node.kind != CHECK_KIND
            {
                next.extend(node.sql_deps.iter().cloned().map(RoundTripNode::Sql));
            }
            next
        }
        RoundTripNode::Sql(key) => {
            let mut next: Vec<RoundTripNode> = context
                .sql
                .graph
                .edges(key, EdgeDirection::Upstream)
                .iter()
                .cloned()
                .map(RoundTripNode::Sql)
                .collect();
            if key.0 == SOURCE_RESOURCE
                && let Some(loader) = source_loaders.get(key.1.as_str())
            {
                next.push(RoundTripNode::Python((*loader).to_owned()));
            }
            next
        }
    })?;
    let last: &RoundTripNode = path.last()?;
    let start: usize = path.iter().position(|node| node == last)?;
    Some(
        path[start..]
            .iter()
            .map(|node| match node {
                RoundTripNode::Python(name) => python_label(context, name),
                RoundTripNode::Sql((kind, name)) => format!("{kind} '{name}'"),
            })
            .collect(),
    )
}

/// Selected managed sources have their loaders selected; a Python node may run without its Python upstream, whose latest persisted result it reads.
pub(crate) fn validate_selected_dependencies(
    context: &SelectionContext<'_>,
    atoms: &BTreeSet<Atom>,
) -> Result<(), SelectorError> {
    let selected: HashSet<&str> = atoms
        .iter()
        .filter_map(|atom| match atom {
            Atom::Python(name) => Some(name.as_str()),
            Atom::Sql(_) => None,
        })
        .collect();
    let loaders: HashMap<&str, &str> = source_loaders(context.sql).into_iter().collect();
    for atom in atoms {
        let Atom::Sql((resource, source)) = atom else {
            continue;
        };
        if resource != SOURCE_RESOURCE {
            continue;
        }
        let Some(&loader) = loaders.get(source.as_str()) else {
            continue;
        };
        if loader == source || selected.contains(loader) {
            continue;
        }
        return Err(failure(format!(
            "Source '{source}' requires loader '{loader}', but that loader was not selected; \
             select it directly or use upstream expansion"
        )));
    }
    Ok(())
}
