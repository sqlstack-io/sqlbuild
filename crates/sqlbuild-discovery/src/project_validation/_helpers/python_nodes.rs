//! Python node dependency checks: known targets, no SQL inputs on loaders, no cycles.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use sqlbuild_core::graph_walk::main::find_cycle::find_cycle as find_dependency_cycle;

use crate::project_validation::_helpers::conflicts::conflict;
use crate::project_validation::models::{DependencyFact, NodeFact, NodeKind, ValidationFailure};

/// `_python_node_dependency_key`: a decorated object's name, else the object itself.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DependencyKey<'a> {
    Name(&'a str),
    Object(Option<usize>),
}

fn dependency_key(dependency: &DependencyFact) -> Option<DependencyKey<'_>> {
    match dependency {
        DependencyFact::SqlResource { .. } => None,
        DependencyFact::Object {
            decorated_name: Some(name),
            ..
        } => Some(DependencyKey::Name(name)),
        DependencyFact::Object { owner, .. } => Some(DependencyKey::Object(*owner)),
    }
}

/// The loaders and assets a dependency key names, as `node_by_dependency_key` maps them.
struct ExecutableNodes<'a> {
    nodes: &'a [NodeFact],
    by_name: HashMap<&'a str, usize>,
}

impl<'a> ExecutableNodes<'a> {
    fn new(nodes: &'a [NodeFact]) -> Self {
        let mut by_name: HashMap<&'a str, usize> = HashMap::new();
        for (index, node) in nodes.iter().enumerate() {
            if node.kind != NodeKind::Check {
                by_name.insert(&node.name, index);
            }
        }
        Self { nodes, by_name }
    }

    fn resolve(&self, key: DependencyKey<'_>) -> Option<usize> {
        match key {
            DependencyKey::Name(name) => self.by_name.get(name).copied(),
            DependencyKey::Object(Some(owner)) if self.nodes[owner].kind != NodeKind::Check => {
                Some(owner)
            }
            DependencyKey::Object(_) => None,
        }
    }
}

/// `_validate_loader_dependencies`: no cycles through loader function dependencies.
pub(crate) fn validate_loader_dependencies(nodes: &[NodeFact]) -> Result<(), ValidationFailure> {
    let next = |index: usize| -> Vec<usize> {
        nodes[index]
            .depends_on
            .iter()
            .filter_map(|dependency| match dependency {
                DependencyFact::Object {
                    owner: Some(owner), ..
                } if nodes[*owner].kind == NodeKind::Loader => Some(*owner),
                _ => None,
            })
            .collect()
    };
    let roots: Vec<usize> = (0..nodes.len())
        .filter(|index| nodes[*index].kind == NodeKind::Loader)
        .collect();
    find_cycle(nodes, &roots, next, "Loader dependency cycle detected")
}

/// `_validate_python_node_dependencies` over loaders and assets.
pub(crate) fn validate_python_node_dependencies(
    nodes: &[NodeFact],
) -> Result<(), ValidationFailure> {
    let executable: ExecutableNodes<'_> = ExecutableNodes::new(nodes);
    let roots: Vec<usize> = (0..nodes.len())
        .filter(|index| nodes[*index].kind != NodeKind::Check)
        .collect();
    for index in &roots {
        let node: &NodeFact = &nodes[*index];
        for dependency in &node.depends_on {
            match dependency {
                DependencyFact::SqlResource { name } => {
                    if node.kind == NodeKind::Loader {
                        return Err(conflict(format!(
                            "Loader '{}' depends on SQL resource '{name}'; loaders may depend on \
                             Python loader/asset functions only",
                            node.name
                        )));
                    }
                }
                DependencyFact::Object { .. } => {
                    let known: bool = dependency_key(dependency)
                        .and_then(|key| executable.resolve(key))
                        .is_some();
                    if known {
                        continue;
                    }
                    if node.kind == NodeKind::Loader {
                        return Err(conflict(format!(
                            "Loader '{}' depends on an unknown loader",
                            node.name
                        )));
                    }
                    return Err(conflict(format!(
                        "Python node '{}' depends on an unknown Python node; use function \
                         references to decorated loaders or assets",
                        node.name
                    )));
                }
            }
        }
    }
    let next = |index: usize| -> Vec<usize> {
        nodes[index]
            .depends_on
            .iter()
            .filter_map(dependency_key)
            .filter_map(|key| executable.resolve(key))
            .collect()
    };
    find_cycle(nodes, &roots, next, "Python node dependency cycle detected")
}

/// `_validate_check_dependencies`: checks depend on loaders and assets, never checks.
pub(crate) fn validate_check_dependencies(nodes: &[NodeFact]) -> Result<(), ValidationFailure> {
    let allowed: HashSet<&str> = nodes
        .iter()
        .filter(|node| node.kind != NodeKind::Check)
        .map(|node| node.name.as_str())
        .collect();
    let checks: HashSet<&str> = nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Check)
        .map(|node| node.name.as_str())
        .collect();
    for check in nodes.iter().filter(|node| node.kind == NodeKind::Check) {
        for dependency in &check.depends_on {
            let Some(key) = dependency_key(dependency) else {
                continue;
            };
            let name: Option<&str> = match key {
                DependencyKey::Name(name) => Some(name),
                DependencyKey::Object(_) => None,
            };
            if name.is_some_and(|name| checks.contains(name)) {
                return Err(conflict(format!(
                    "Check '{}' depends on another check; checks may depend on loaders, \
                     assets, models, sources, and seeds only",
                    check.name
                )));
            }
            if !name.is_some_and(|name| allowed.contains(name)) {
                return Err(conflict(format!(
                    "Check '{}' depends on an unknown Python node; use function references to \
                     decorated loaders or assets",
                    check.name
                )));
            }
        }
    }
    Ok(())
}

/// Python's recursive `visit`: depth-first from each root, sharing visited names across roots.
fn find_cycle(
    nodes: &[NodeFact],
    roots: &[usize],
    next: impl Fn(usize) -> Vec<usize>,
    label: &str,
) -> Result<(), ValidationFailure> {
    let named = |index: usize| NamedNode {
        name: &nodes[index].name,
        index,
    };
    let Some(cycle) = find_dependency_cycle(roots.iter().map(|root| named(*root)), |node| {
        next(node.index)
            .into_iter()
            .map(named)
            .collect::<Vec<NamedNode<'_>>>()
    }) else {
        return Ok(());
    };
    let names: Vec<&str> = cycle.iter().map(|node| node.name).collect();
    Err(conflict(format!("{label}: {}", names.join(" -> "))))
}

/// A node position whose identity in a cycle search is its name.
#[derive(Clone, Copy)]
struct NamedNode<'a> {
    name: &'a str,
    index: usize,
}

impl PartialEq for NamedNode<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for NamedNode<'_> {}

impl Hash for NamedNode<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}
