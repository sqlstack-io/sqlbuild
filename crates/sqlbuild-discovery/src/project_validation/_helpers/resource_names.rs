//! One namespace across resource kinds, orchestration declarations included.

use std::collections::{HashMap, HashSet};

use crate::project_validation::_helpers::conflicts::{conflict, schema_seeds};
use crate::project_validation::_helpers::identities::block_or_file_name;
use crate::project_validation::constants::ORCHESTRATION_RENAME_ADVICE;
use crate::project_validation::models::{
    DeclaredName, NamedPath, NodeKind, ProjectFacts, ValidationFailure,
};

/// Reject the first resource name declared twice across models, sources, seeds and the rest.
pub(crate) fn validate_unique_project_resource_names(
    facts: &ProjectFacts,
) -> Result<(), ValidationFailure> {
    let seeds: Vec<NamedPath> = schema_seeds(facts);
    let mut seen: HashMap<&str, DeclaredName<'_>> = HashMap::new();
    for declared in declarations(facts, &seeds) {
        if let Some(first) = seen.get(declared.name) {
            return Err(name_conflict(first, &declared));
        }
        seen.insert(declared.name, declared);
    }
    Ok(())
}

/// Every declared name in the order conflicts are reported.
fn declarations<'a>(facts: &'a ProjectFacts, seeds: &'a [NamedPath]) -> Vec<DeclaredName<'a>> {
    let mut declared: Vec<DeclaredName<'a>> = Vec::new();
    for model in &facts.models {
        declared.push(DeclaredName {
            name: &model.file.name,
            kind: "model",
            path: &model.file.path,
            orchestration: false,
        });
    }
    let mut managed_sources: HashSet<&str> = HashSet::new();
    for file in &facts.sources {
        for entry in &file.entries {
            if entry.managed {
                managed_sources.insert(&entry.name);
            }
            declared.push(DeclaredName {
                name: &entry.name,
                kind: "source",
                path: &file.path,
                orchestration: false,
            });
        }
    }
    for seed in seeds {
        declared.push(DeclaredName {
            name: &seed.name,
            kind: "seed",
            path: &seed.path,
            orchestration: false,
        });
    }
    for function in facts.sql_functions.iter().chain(&facts.python_functions) {
        declared.push(DeclaredName {
            name: &function.name,
            kind: "function",
            path: &function.path,
            orchestration: false,
        });
    }
    for node in &facts.nodes {
        if node.kind == NodeKind::Loader && managed_sources.contains(node.name.as_str()) {
            continue;
        }
        declared.push(DeclaredName {
            name: &node.name,
            kind: node.kind.label(),
            path: &node.path,
            orchestration: false,
        });
    }
    for provider in &facts.providers {
        declared.push(DeclaredName {
            name: &provider.name,
            kind: "provider",
            path: &provider.path,
            orchestration: false,
        });
    }
    for hook in &facts.sql_hooks {
        declared.push(DeclaredName {
            name: &hook.name,
            kind: "sql hook",
            path: &hook.path,
            orchestration: false,
        });
    }
    for hook in &facts.hook_functions {
        declared.push(DeclaredName {
            name: &hook.name,
            kind: "python hook",
            path: &hook.path,
            orchestration: false,
        });
    }
    for scenario in &facts.scenarios {
        declared.push(DeclaredName {
            name: &scenario.name,
            kind: "scenario",
            path: &scenario.path,
            orchestration: false,
        });
    }
    let mut test_names: HashSet<&str> = HashSet::new();
    for test in &facts.tests {
        for block in &test.blocks {
            let name: &str = block_or_file_name(block.name.as_deref(), &test.file.name);
            if test_names.insert(name) {
                declared.push(DeclaredName {
                    name,
                    kind: "unit test",
                    path: &test.file.path,
                    orchestration: false,
                });
            }
        }
    }
    for declaration in &facts.orchestration {
        declared.push(DeclaredName {
            name: &declaration.name,
            kind: declaration.kind,
            path: &declaration.path,
            orchestration: true,
        });
    }
    declared
}

/// Name both declarations and say how to rename one of them.
fn name_conflict(first: &DeclaredName<'_>, second: &DeclaredName<'_>) -> ValidationFailure {
    let advice: &str = if first.orchestration || second.orchestration {
        ORCHESTRATION_RENAME_ADVICE
    } else {
        ""
    };
    conflict(format!(
        "Project resource name '{}' is declared as both {} in {} and {} in {}; model, source, seed, function, loader, task, asset, check, provider, \
         hook, unit test, scenario, job, job group, sensor, and calendar names must be \
         globally unique; rename one of them{advice}",
        first.name, first.kind, first.path, second.kind, second.path
    ))
}
