//! One namespace across resource kinds, orchestration declarations included.

use std::collections::{HashMap, HashSet};

use crate::project_validation::_helpers::conflicts::{conflict, schema_seeds};
use crate::project_validation::_helpers::identities::block_or_file_name;
use crate::project_validation::models::{NodeKind, ProjectFacts, ValidationFailure};

/// Reject the first resource name declared twice across models, sources, seeds and the rest.
pub(crate) fn validate_unique_project_resource_names(
    facts: &ProjectFacts,
) -> Result<(), ValidationFailure> {
    let mut seen: HashMap<String, (String, String)> = HashMap::new();
    let mut declare = |name: &str, kind: &str, path: &str| -> Result<(), ValidationFailure> {
        if let Some((existing_kind, existing_path)) = seen.get(name) {
            return Err(conflict(format!(
                "Project resource name '{name}' is declared as both {existing_kind} in \
                 {existing_path} and {kind} in {path}; model, source, seed, function, loader, \
                 task, asset, check, provider, hook, unit test, scenario, job, job group, sensor, \
                 and calendar schedule names must be globally unique"
            )));
        }
        seen.insert(name.to_owned(), (kind.to_owned(), path.to_owned()));
        Ok(())
    };
    for model in &facts.models {
        declare(&model.file.name, "model", &model.file.path)?;
    }
    let mut managed_sources: HashSet<&str> = HashSet::new();
    for file in &facts.sources {
        for entry in &file.entries {
            if entry.managed {
                managed_sources.insert(&entry.name);
            }
            declare(&entry.name, "source", &file.path)?;
        }
    }
    for seed in schema_seeds(facts) {
        declare(&seed.name, "seed", &seed.path)?;
    }
    for function in facts.sql_functions.iter().chain(&facts.python_functions) {
        declare(&function.name, "function", &function.path)?;
    }
    for node in &facts.nodes {
        if node.kind == NodeKind::Loader && managed_sources.contains(node.name.as_str()) {
            continue;
        }
        declare(&node.name, node.kind.label(), &node.path)?;
    }
    for provider in &facts.providers {
        declare(&provider.name, "provider", &provider.path)?;
    }
    for hook in &facts.sql_hooks {
        declare(&hook.name, "sql hook", &hook.path)?;
    }
    for hook in &facts.hook_functions {
        declare(&hook.name, "python hook", &hook.path)?;
    }
    for scenario in &facts.scenarios {
        declare(&scenario.name, "scenario", &scenario.path)?;
    }
    let mut test_names: HashSet<&str> = HashSet::new();
    for test in &facts.tests {
        for block in &test.blocks {
            let name: &str = block_or_file_name(block.name.as_deref(), &test.file.name);
            if !test_names.insert(name) {
                continue;
            }
            declare(name, "unit test", &test.file.path)?;
        }
    }
    for declaration in &facts.orchestration {
        declare(&declaration.name, declaration.kind, &declaration.path)?;
    }
    Ok(())
}
