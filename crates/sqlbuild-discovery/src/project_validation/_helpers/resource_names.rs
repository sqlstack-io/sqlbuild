//! One namespace across resource kinds, orchestration declarations included.

use std::collections::{HashMap, HashSet};

use crate::project_validation::_helpers::conflicts::{conflict, schema_seeds};
use crate::project_validation::_helpers::identities::block_or_file_name;
use crate::project_validation::constants::ORCHESTRATION_RENAME_ADVICE;
use crate::project_validation::models::{NodeKind, ProjectFacts, ValidationFailure};

/// Reject the first resource name declared twice across models, sources, seeds and the rest.
pub(crate) fn validate_unique_project_resource_names(
    facts: &ProjectFacts,
) -> Result<(), ValidationFailure> {
    let mut seen: HashMap<String, Declared> = HashMap::new();
    for model in &facts.models {
        declare(
            &mut seen,
            (&model.file.name, "model", &model.file.path),
            false,
        )?;
    }
    let mut managed_sources: HashSet<&str> = HashSet::new();
    for file in &facts.sources {
        for entry in &file.entries {
            if entry.managed {
                managed_sources.insert(&entry.name);
            }
            declare(&mut seen, (&entry.name, "source", &file.path), false)?;
        }
    }
    for seed in schema_seeds(facts) {
        declare(&mut seen, (&seed.name, "seed", &seed.path), false)?;
    }
    for function in facts.sql_functions.iter().chain(&facts.python_functions) {
        declare(
            &mut seen,
            (&function.name, "function", &function.path),
            false,
        )?;
    }
    for node in &facts.nodes {
        if node.kind == NodeKind::Loader && managed_sources.contains(node.name.as_str()) {
            continue;
        }
        declare(
            &mut seen,
            (&node.name, node.kind.label(), &node.path),
            false,
        )?;
    }
    for provider in &facts.providers {
        declare(
            &mut seen,
            (&provider.name, "provider", &provider.path),
            false,
        )?;
    }
    for hook in &facts.sql_hooks {
        declare(&mut seen, (&hook.name, "sql hook", &hook.path), false)?;
    }
    for hook in &facts.hook_functions {
        declare(&mut seen, (&hook.name, "python hook", &hook.path), false)?;
    }
    for scenario in &facts.scenarios {
        declare(
            &mut seen,
            (&scenario.name, "scenario", &scenario.path),
            false,
        )?;
    }
    let mut test_names: HashSet<&str> = HashSet::new();
    for test in &facts.tests {
        for block in &test.blocks {
            let name: &str = block_or_file_name(block.name.as_deref(), &test.file.name);
            if !test_names.insert(name) {
                continue;
            }
            declare(&mut seen, (name, "unit test", &test.file.path), false)?;
        }
    }
    for declaration in &facts.orchestration {
        declare(
            &mut seen,
            (&declaration.name, declaration.kind, &declaration.path),
            true,
        )?;
    }
    Ok(())
}

/// A name's first declaration: its kind, its file and whether it is an orchestration declaration.
type Declared = (String, String, bool);

/// Record one declaration, or name both declarations and say how to rename one of them.
fn declare(
    seen: &mut HashMap<String, Declared>,
    (name, kind, path): (&str, &str, &str),
    orchestration: bool,
) -> Result<(), ValidationFailure> {
    if let Some((existing_kind, existing_path, existing_orchestration)) = seen.get(name) {
        let advice: &str = if orchestration || *existing_orchestration {
            ORCHESTRATION_RENAME_ADVICE
        } else {
            ""
        };
        return Err(conflict(format!(
            "Project resource name '{name}' is declared as both {existing_kind} in \
             {existing_path} and {kind} in {path}; model, source, seed, function, loader, task, \
             asset, check, provider, hook, unit test, scenario, job, job group, sensor, and \
             calendar schedule names must be globally unique; rename one of them{advice}"
        )));
    }
    seen.insert(
        name.to_owned(),
        (kind.to_owned(), path.to_owned(), orchestration),
    );
    Ok(())
}
