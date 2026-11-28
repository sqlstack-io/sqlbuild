//! Loader and Python node names, dependencies, and the sources that name loaders.

use std::collections::{HashMap, HashSet};

use crate::project_validation::_helpers::conflicts::{conflict, node_path, unique_named};
use crate::project_validation::_helpers::python_nodes::{
    validate_check_dependencies, validate_loader_dependencies, validate_python_node_dependencies,
};
use crate::project_validation::models::{
    NamedPath, NodeFact, NodeKind, ProjectFacts, SourceFileFact, ValidationFailure,
};

/// Validate loader and node names, node dependencies, then the loaders sources name.
pub(crate) fn validate_loaders(facts: &ProjectFacts) -> Result<(), ValidationFailure> {
    let loaders: Vec<&NodeFact> = facts
        .nodes
        .iter()
        .filter(|node| node.kind == NodeKind::Loader)
        .collect();
    let loader_paths: Vec<NamedPath> = loaders.iter().map(|node| node_path(node)).collect();
    unique_named(&loader_paths, |name, existing, path| {
        format!("Duplicate source loader found for '{name}' in {existing} and {path}")
    })?;
    let node_paths: Vec<NamedPath> = facts.nodes.iter().map(node_path).collect();
    unique_named(&node_paths, |name, existing, path| {
        format!(
            "Duplicate Python node found for '{name}' in {existing} and {path}; loader, \
             asset, and check names must be globally unique"
        )
    })?;
    let mut load_paths: Vec<NamedPath> = Vec::new();
    for file in &facts.sources {
        if let Some(name) = &file.load {
            load_paths.push(NamedPath {
                name: name.clone(),
                path: file.path.clone(),
            });
        }
    }
    unique_named(&load_paths, |name, existing, path| {
        format!(
            "Duplicate load '{name}' in {existing} and {path}; each sources file declares at \
             most one load, and load names are unique"
        )
    })?;
    source_loader_name_collisions(&facts.sources, &loaders)?;
    validate_loader_dependencies(&facts.nodes)?;
    validate_python_node_dependencies(&facts.nodes)?;
    validate_check_dependencies(&facts.nodes)?;
    source_loader_references(&facts.sources, &loaders)?;
    terminal_loader_config_ownership(&facts.sources, &loaders)
}

fn source_loader_name_collisions(
    sources: &[SourceFileFact],
    loaders: &[&NodeFact],
) -> Result<(), ValidationFailure> {
    let loader_paths: HashMap<&str, &str> = loaders
        .iter()
        .map(|loader| (loader.name.as_str(), loader.path.as_str()))
        .collect();
    for file in sources {
        for entry in &file.entries {
            let Some(loader_path) = loader_paths.get(entry.name.as_str()) else {
                continue;
            };
            if entry.managed && entry.loader.as_deref() == Some(entry.name.as_str()) {
                continue;
            }
            return Err(conflict(format!(
                "Source '{name}' in {path} conflicts with loader '{name}' in {loader_path}; \
                 source and loader names must be globally unique unless the source is managed",
                name = entry.name,
                path = file.path,
            )));
        }
    }
    Ok(())
}

fn source_loader_references(
    sources: &[SourceFileFact],
    loaders: &[&NodeFact],
) -> Result<(), ValidationFailure> {
    let loader_names: HashSet<&str> = loaders.iter().map(|loader| loader.name.as_str()).collect();
    for file in sources {
        for entry in &file.entries {
            let expected: String = entry.name.clone();
            if entry.managed && entry.loader.as_deref() != Some(expected.as_str()) {
                return Err(conflict(format!(
                    "Managed source '{}' in {} must use terminal loader '{expected}'",
                    entry.name, file.path
                )));
            }
            if entry.managed && !loader_names.contains(expected.as_str()) {
                return Err(conflict(format!(
                    "Managed source '{}' in {} requires loader '{expected}'",
                    entry.name, file.path
                )));
            }
            match &entry.loader {
                Some(loader) if !loader_names.contains(loader.as_str()) => {
                    return Err(conflict(format!(
                        "Source '{}' in {} references unknown loader '{loader}'",
                        entry.name, file.path
                    )));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn terminal_loader_config_ownership(
    sources: &[SourceFileFact],
    loaders: &[&NodeFact],
) -> Result<(), ValidationFailure> {
    let by_name: HashMap<&str, &NodeFact> = loaders
        .iter()
        .map(|loader| (loader.name.as_str(), *loader))
        .collect();
    for file in sources {
        for entry in &file.entries {
            let Some(loader) = &entry.loader else {
                continue;
            };
            let configured: bool = by_name
                .get(loader.as_str())
                .is_some_and(|loader| loader.decorator_config);
            if configured {
                return Err(conflict(format!(
                    "Source '{}' references loader '{loader}', which sets destination or columns; \
                     the source names its table and declares its columns in its sources file, so \
                     @loader can't",
                    entry.name
                )));
            }
        }
    }
    Ok(())
}
