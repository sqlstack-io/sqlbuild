//! Python's unified selector resolution across SQL resources and Python nodes.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::convert::Infallible;

use sqlbuild_core::graph_walk::main::breadth_first_reach::breadth_first_reach;
use sqlbuild_core::text::main::glob_match::glob_match;

use crate::assembly::project::types::ObjectKey;
use crate::graph::errors::SelectorError;
use crate::graph::main::build_resources::build_resources;
use crate::graph::main::edge_keys::edge_keys;
use crate::graph::main::parse_selector::parse_selector;
use crate::graph::main::resolve_selectors::resolve_selectors;
use crate::graph::models::{BuildResources, EdgeDirection, ParsedSelector};
use crate::python_selection::_helpers::nodes::{
    Names, error, folder_root, is_pattern, select_nodes, selector_folder, unit_test_error,
};
use crate::python_selection::_helpers::validation::{
    source_loaders, terminal_loaders, validate_boundaries, validate_selected_dependencies,
};
use crate::python_selection::constants::{
    ASSET_KIND, CHECK_KIND, EXPANSION_MARKER, INTERSECTION_SEPARATOR, JOB_KIND, LOADER_KIND,
    NAME_KIND, PATH_KIND, PATH_ROOT_CODE, PATH_ROOT_ERROR, PLANNER_DEFAULT_CODE, SEED_KIND,
    SOURCE_KIND, SOURCE_RESOURCE, SQL_MODEL_PATH_ROOT, TAG_KIND, TASK_KIND, TEST_KIND,
    UNKNOWN_NAME_CODE, UNKNOWN_TAG_CODE,
};
use crate::python_selection::models::{Atom, PythonSelectionNode, RunSelection, SelectionContext};
use sqlbuild_core::constants::PYTHON_NODE_ROOT as PYTHON_NODE_PATH_ROOT;

type Atoms = BTreeSet<Atom>;
type Resolved = Result<Atoms, SelectorError>;

fn python_atoms(names: Names) -> Atoms {
    names.into_iter().map(Atom::Python).collect()
}

fn sql_atoms(keys: impl IntoIterator<Item = ObjectKey>) -> Atoms {
    keys.into_iter().map(Atom::Sql).collect()
}

fn sql_keys(atoms: &Atoms) -> Vec<ObjectKey> {
    atoms
        .iter()
        .filter_map(|atom| match atom {
            Atom::Sql(key) => Some(key.clone()),
            Atom::Python(_) => None,
        })
        .collect()
}

/// Python's `resolve_python_sql_run_selectors`.
pub(crate) fn run_selection(
    context: &SelectionContext<'_>,
    select: &[String],
    exclude: &[String],
) -> Result<RunSelection, SelectorError> {
    validate_boundaries(context)?;
    let mut selected: Atoms = if select.is_empty() {
        let mut every: Atoms =
            sql_atoms(context.sql.graph.names.iter().map(|(_, key)| key.clone()));
        every.extend(
            context
                .nodes
                .by_name
                .keys()
                .map(|name| Atom::Python((*name).to_owned())),
        );
        every
    } else {
        groups(context, select)?
    };
    let excluded: Atoms = groups(context, exclude)?;
    selected = selected.difference(&excluded).cloned().collect();
    let required: Atoms = sql_atoms(build_resources(
        context.sql.graph,
        &sql_keys(&selected),
        BuildResources::SELECTION,
    ));
    selected.extend(required);
    let is_check =
        |atom: &Atom| matches!(atom, Atom::Python(name) if context.nodes.is_kind(name, CHECK_KIND));
    let checks: Vec<&str> = selected
        .iter()
        .filter(|atom| is_check(atom))
        .filter_map(|atom| match atom {
            Atom::Python(name) => Some(name.as_str()),
            Atom::Sql(_) => None,
        })
        .collect();
    if !select.is_empty() && !checks.is_empty() {
        return Err(error(
            PLANNER_DEFAULT_CODE,
            format!(
                "Python checks are not selectable here: {}. Use sqb check instead.",
                checks.join(", ")
            ),
        ));
    }
    let runnable: Atoms = selected
        .into_iter()
        .filter(|atom| !is_check(atom))
        .collect();
    validate_selected_dependencies(context, &runnable)?;
    Ok(selection(context, sql_keys(&runnable), &runnable))
}

/// The SQL keys, with the source each selected intermediate loader writes, and the selected Python node names.
fn selection(context: &SelectionContext<'_>, sql: Vec<ObjectKey>, atoms: &Atoms) -> RunSelection {
    let terminal: HashSet<&str> = context
        .sql
        .sources
        .iter()
        .filter_map(|(_, loader)| loader.as_deref())
        .collect();
    let mut keys: BTreeSet<ObjectKey> = sql.into_iter().collect();
    let mut names: Vec<String> = Vec::new();
    for atom in atoms {
        let Atom::Python(name) = atom else {
            continue;
        };
        if context.nodes.is_kind(name, LOADER_KIND) && !terminal.contains(name.as_str()) {
            keys.insert((SOURCE_RESOURCE.to_owned(), name.clone()));
        }
        names.push(name.clone());
    }
    RunSelection {
        sql_keys: keys.into_iter().collect(),
        python_node_names: names,
    }
}

/// The SQL keys with their build functions and the Python node names, checks included, that selectors name; nothing is refused.
pub(crate) fn node_selection(
    context: &SelectionContext<'_>,
    select: &[String],
    exclude: &[String],
) -> Result<RunSelection, SelectorError> {
    validate_boundaries(context)?;
    let selected: Atoms = groups(context, select)?
        .difference(&groups(context, exclude)?)
        .cloned()
        .collect();
    let sql: Vec<ObjectKey> = build_resources(
        context.sql.graph,
        &sql_keys(&selected),
        BuildResources::SELECTION,
    );
    Ok(selection(context, sql, &selected))
}

fn groups(context: &SelectionContext<'_>, raw_groups: &[String]) -> Resolved {
    let mut resolved: Atoms = Atoms::new();
    for group in raw_groups {
        for token in group.split_whitespace() {
            resolved.extend(intersection(context, token)?);
        }
    }
    Ok(resolved)
}

fn intersection(context: &SelectionContext<'_>, token: &str) -> Resolved {
    let parts: Vec<Atoms> = token
        .split(INTERSECTION_SEPARATOR)
        .map(|part| single(context, part))
        .collect::<Result<_, _>>()?;
    let mut parts = parts.into_iter();
    let mut result: Atoms = parts.next().unwrap_or_default();
    for part in parts {
        result = result.intersection(&part).cloned().collect();
    }
    Ok(result)
}

fn single(context: &SelectionContext<'_>, raw: &str) -> Resolved {
    let (upstream, downstream) = match parse_selector(raw)? {
        ParsedSelector::Path { .. } => return single_side(context, raw),
        ParsedSelector::Kind {
            upstream,
            downstream,
            ..
        } => (upstream, downstream),
    };
    if !upstream && !downstream {
        return single_side(context, raw);
    }
    let core: &str = raw
        .trim()
        .trim_start_matches(EXPANSION_MARKER)
        .trim_end_matches(EXPANSION_MARKER);
    let mut atoms: Atoms = Atoms::new();
    if upstream {
        let resolved: Atoms = single_side(context, &format!("{EXPANSION_MARKER}{core}"))?;
        let dependencies: Atoms = sql_dependency_upstream(context, &resolved);
        atoms.extend(resolved);
        atoms.extend(dependencies);
    }
    if downstream {
        let resolved: Atoms = single_side(context, &format!("{core}{EXPANSION_MARKER}"))?;
        let dependents: Atoms = sql_dependent_python(context, &resolved);
        atoms.extend(resolved);
        atoms.extend(dependents);
    }
    Ok(atoms)
}

/// The SQL resources selected Python nodes declare, with their SQL upstream and loaders.
fn sql_dependency_upstream(context: &SelectionContext<'_>, atoms: &Atoms) -> Atoms {
    let starts: BTreeSet<&ObjectKey> = atoms
        .iter()
        .filter_map(|atom| match atom {
            Atom::Python(name) => context.nodes.node(name),
            Atom::Sql(_) => None,
        })
        .flat_map(|node| node.sql_deps.iter())
        .collect();
    if starts.is_empty() {
        return Atoms::new();
    }
    let mut visited: HashSet<&ObjectKey> = starts.iter().copied().collect();
    let Ok(()) = breadth_first_reach(starts, None, &mut visited, |current| {
        Ok::<_, Infallible>(edge_keys(
            context.sql.graph,
            current,
            EdgeDirection::Upstream,
        ))
    });
    let selected: Atoms = sql_atoms(visited.into_iter().cloned());
    let loaders: Atoms = terminal_loader_atoms(context, &selected);
    selected.into_iter().chain(loaders).collect()
}

fn declares_any(node: &PythonSelectionNode, keys: &HashSet<&ObjectKey>) -> bool {
    node.sql_deps.iter().any(|key| keys.contains(key))
}

/// Runnable Python nodes declaring a selected SQL resource, with their downstream nodes.
fn sql_dependent_python(context: &SelectionContext<'_>, atoms: &Atoms) -> Atoms {
    let selected: HashSet<&ObjectKey> = atoms
        .iter()
        .filter_map(|atom| match atom {
            Atom::Sql(key) => Some(key),
            Atom::Python(_) => None,
        })
        .collect();
    let dependents: BTreeSet<&str> = context
        .nodes
        .graph
        .nodes
        .iter()
        .filter(|node| node.kind != CHECK_KIND)
        .filter(|node| declares_any(node, &selected))
        .map(|node| node.name.as_str())
        .collect();
    if dependents.is_empty() {
        return Atoms::new();
    }
    let mut names: Names = context
        .nodes
        .closure(dependents.iter().copied(), EdgeDirection::Downstream);
    names.extend(dependents.iter().map(|name| (*name).to_owned()));
    python_atoms(
        names
            .into_iter()
            .filter(|name| !context.nodes.is_kind(name, CHECK_KIND))
            .collect(),
    )
}

fn single_side(context: &SelectionContext<'_>, raw: &str) -> Resolved {
    let (kind, value, upstream) = match parse_selector(raw)? {
        ParsedSelector::Path { upstream, .. } => {
            return Ok(with_loaders(context, resolve_sql(context, raw)?, upstream));
        }
        ParsedSelector::Kind {
            kind,
            value,
            upstream,
            ..
        } => (kind, value, upstream),
    };
    let atoms: Atoms = match kind.as_str() {
        TEST_KIND => return Err(unit_test_error(raw)),
        PATH_KIND => resolve_path(context, raw, &value)?,
        SEED_KIND | SOURCE_KIND | JOB_KIND => resolve_sql(context, raw)?,
        TASK_KIND | ASSET_KIND | LOADER_KIND | CHECK_KIND => {
            return resolve_typed(context, raw, (&kind, &value));
        }
        TAG_KIND => resolve_tag(context, raw, &value)?,
        NAME_KIND => resolve_name(context, raw, &value)?,
        _ => {
            return Err(error(
                PLANNER_DEFAULT_CODE,
                format!("unsupported selector '{raw}'"),
            ));
        }
    };
    Ok(with_loaders(context, atoms, upstream))
}

fn with_loaders(context: &SelectionContext<'_>, atoms: Atoms, upstream: bool) -> Atoms {
    if !upstream {
        return atoms;
    }
    let loaders: Atoms = terminal_loader_atoms(context, &atoms);
    atoms.into_iter().chain(loaders).collect()
}

fn resolve_sql(context: &SelectionContext<'_>, raw: &str) -> Resolved {
    Ok(sql_atoms(resolve_selectors(
        context.sql.graph,
        &[raw.to_owned()],
        &[],
    )?))
}

fn resolve_python(context: &SelectionContext<'_>, raw: &str) -> Resolved {
    Ok(python_atoms(select_nodes(
        &context.nodes,
        &[raw.to_owned()],
        &[],
    )?))
}

fn resolve_typed(
    context: &SelectionContext<'_>,
    raw: &str,
    (kind, value): (&str, &str),
) -> Resolved {
    if kind == LOADER_KIND {
        let terminal: HashMap<&str, &str> = terminal_loaders(context.sql);
        let matched: Option<&&str> = terminal.keys().filter(|name| glob_match(name, value)).min();
        if let Some(&loader) = matched {
            return Err(error(
                UNKNOWN_NAME_CODE,
                format!(
                    "loader selector '{value}' matches managed source loader '{loader}'; \
                     select it as source:{}",
                    terminal[loader]
                ),
            ));
        }
    }
    resolve_python(context, raw)
}

fn resolve_tag(context: &SelectionContext<'_>, raw: &str, tag: &str) -> Resolved {
    let mut atoms: Atoms = Atoms::new();
    for resolved in [resolve_sql(context, raw), resolve_python(context, raw)] {
        match resolved {
            Ok(found) => atoms.extend(found),
            Err(failure) if failure.code == UNKNOWN_TAG_CODE => {}
            Err(failure) => return Err(failure),
        }
    }
    if atoms.is_empty() {
        return Err(error(
            PLANNER_DEFAULT_CODE,
            format!("no SQL resources or Python nodes found with tag '{tag}'"),
        ));
    }
    Ok(atoms)
}

fn resolve_path(context: &SelectionContext<'_>, raw: &str, value: &str) -> Resolved {
    let folder: String = selector_folder(value);
    match folder_root(&folder) {
        SQL_MODEL_PATH_ROOT => resolve_sql(context, raw),
        PYTHON_NODE_PATH_ROOT => resolve_python(context, raw),
        _ => Err(error(PATH_ROOT_CODE, PATH_ROOT_ERROR.to_owned())),
    }
}

fn both_error(name: &str) -> SelectorError {
    error(
        PLANNER_DEFAULT_CODE,
        format!(
            "selector name '{name}' matches both a SQL resource and a Python node; \
             resource names must be globally unique"
        ),
    )
}

fn is_job(context: &SelectionContext<'_>, value: &str) -> bool {
    context
        .sql
        .graph
        .jobs
        .as_ref()
        .is_some_and(|jobs| jobs.contains_key(value))
}

fn unknown_name(value: &str) -> SelectorError {
    error(
        UNKNOWN_NAME_CODE,
        format!("unknown selector name '{value}'"),
    )
}

fn resolve_name(context: &SelectionContext<'_>, raw: &str, value: &str) -> Resolved {
    let terminal: HashMap<&str, &str> = terminal_loaders(context.sql);
    if !is_pattern(value) {
        let sql: bool = context.sql_names.contains_key(value);
        let python: bool = context.nodes.node(value).is_some();
        return match (sql, python) {
            (true, _) if terminal.contains_key(value) => resolve_sql(context, raw),
            (true, true) => Err(both_error(value)),
            (true, false) => resolve_sql(context, raw),
            (false, true) => resolve_python(context, raw),
            (false, false) if is_job(context, value) => resolve_sql(context, raw),
            (false, false) => Err(unknown_name(value)),
        };
    }
    let sql_names: BTreeSet<&str> = context
        .sql
        .graph
        .names
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| glob_match(name, value))
        .collect();
    let python_names: BTreeSet<&str> = context
        .nodes
        .by_name
        .keys()
        .copied()
        .filter(|name| glob_match(name, value))
        .collect();
    if let Some(conflict) = sql_names
        .intersection(&python_names)
        .find(|name| !terminal.contains_key(*name))
    {
        return Err(both_error(conflict));
    }
    let mut atoms: Atoms = Atoms::new();
    if !sql_names.is_empty() {
        atoms.extend(resolve_sql(context, raw)?);
    }
    if !python_names.is_empty() {
        let duplicates: HashSet<&str> = sql_names
            .iter()
            .copied()
            .filter(|name| terminal.contains_key(name))
            .collect();
        atoms.extend(resolve_python(context, raw)?.into_iter().filter(
            |atom| !matches!(atom, Atom::Python(name) if duplicates.contains(name.as_str())),
        ));
    }
    if atoms.is_empty() {
        return Err(unknown_name(value));
    }
    Ok(atoms)
}

/// Python's `_required_terminal_loader_atoms`: loaders of selected sources and their upstream.
fn terminal_loader_atoms(context: &SelectionContext<'_>, atoms: &Atoms) -> Atoms {
    let loaders: HashMap<&str, &str> = source_loaders(context.sql).into_iter().collect();
    let mut required: Atoms = Atoms::new();
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
        if context.nodes.node(loader).is_none() {
            continue;
        }
        required.insert(Atom::Python(loader.to_owned()));
        required.extend(python_atoms(
            context.nodes.closure([loader], EdgeDirection::Upstream),
        ));
    }
    required
}
