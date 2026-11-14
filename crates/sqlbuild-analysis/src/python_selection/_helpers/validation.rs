//! Python's SQL/Python graph boundary rules and selected-dependency checks.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::graph::errors::SelectorError;
use crate::graph::models::EdgeDirection;
use crate::python_selection::_helpers::nodes::error;
use crate::python_selection::constants::{
    ASSET_KIND, CHECK_KIND, LOADER_KIND, MODEL_RESOURCE, PLANNER_DEFAULT_CODE, SOURCE_RESOURCE,
    SQL_MODEL_DEPENDENCY_TYPES, TASK_KIND,
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
            if node.kind == TASK_KIND || node.kind == ASSET_KIND {
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

fn validate_loader_upstreams(context: &SelectionContext<'_>) -> Result<(), SelectorError> {
    for loader in context
        .nodes
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind == LOADER_KIND)
    {
        for upstream_name in context
            .nodes
            .closure([loader.name.as_str()], EdgeDirection::Upstream)
        {
            let Some(upstream) = context.nodes.node(&upstream_name) else {
                continue;
            };
            if (upstream.kind == TASK_KIND || upstream.kind == ASSET_KIND)
                && !upstream.sql_deps.is_empty()
            {
                return Err(failure(format!(
                    "Loader '{}' depends on Python node '{}' which depends on SQL; \
                     Python-to-SQL writes must flow through a pre-SQL task/asset -> loader -> \
                     source path",
                    loader.name, upstream.name
                )));
            }
        }
    }
    Ok(())
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
