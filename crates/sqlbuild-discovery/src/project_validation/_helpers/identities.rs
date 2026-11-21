//! Python's `validate_discovered_resource_identities`: canonical identities, in discovery order.

use crate::models::{DiscoveryFailure, FailureKind};
use crate::project_validation::models::{
    AuditFileFact, NamedPath, NodeKind, ProjectFacts, ValidationFailure,
};
use sqlbuild_model_config::resource_identities::main::resource_identity_failure::resource_identity_failure;

/// One identity to check: its name, the kind the error names, its path and whether it is private.
struct Candidate<'a> {
    name: &'a str,
    kind: &'a str,
    path: &'a str,
    private: bool,
}

/// The identities to check, in the order Python's `validate_discovered_resource_identities` lists them.
struct Candidates<'a> {
    list: Vec<Candidate<'a>>,
}

impl<'a> Candidates<'a> {
    fn push(&mut self, name: &'a str, kind: &'a str, path: &'a str) {
        self.list.push(Candidate {
            name,
            kind,
            path,
            private: false,
        });
    }

    fn push_named(&mut self, entries: &'a [NamedPath], kind: &'a str) {
        for entry in entries {
            self.push(&entry.name, kind, &entry.path);
        }
    }

    /// A generic audit is named by its stem, each block naming an instance; others by the block.
    fn push_audit(&mut self, audit: &'a AuditFileFact, block_name: Option<&'a str>) {
        let path: &str = &audit.file.path;
        if audit.generic {
            self.push(&audit.file.name, "audit", path);
            if let Some(instance) = block_name {
                self.push(instance, "generic audit instance", path);
            }
        } else {
            self.push(
                block_or_file_name(block_name, &audit.file.name),
                "audit",
                path,
            );
        }
    }

    fn push_declarations(&mut self, declarations: &'a [NamedPath], kind: &'a str, private: bool) {
        for declaration in declarations {
            self.list.push(Candidate {
                name: &declaration.name,
                kind,
                path: &declaration.path,
                private,
            });
        }
    }
}

/// Reject the first identity that is not canonical lowercase snake_case.
pub(crate) fn validate_identities(facts: &ProjectFacts) -> Result<(), ValidationFailure> {
    for candidate in candidates(facts) {
        if let Some((corrected, help)) =
            resource_identity_failure(candidate.name, candidate.private)
        {
            return Err(ValidationFailure::Discovery(DiscoveryFailure {
                kind: FailureKind::ResourceIdentity,
                message: format!(
                    "Invalid {} identity '{}' in {}; use snake_case '{corrected}'",
                    candidate.kind, candidate.name, candidate.path
                ),
                help: Some(help),
            }));
        }
    }
    Ok(())
}

fn candidates(facts: &ProjectFacts) -> Vec<Candidate<'_>> {
    let mut candidates: Candidates<'_> = Candidates { list: Vec::new() };
    for model in &facts.models {
        candidates.push(&model.file.name, "model", &model.file.path);
        candidates.push_declarations(&model.local_declarations, "model-local declaration", true);
    }
    candidates.push_named(&facts.sql_functions, "function");
    candidates.push_named(&facts.python_functions, "function");
    candidates.push_named(&facts.sql_hooks, "SQL hook");
    for seed in &facts.seeds {
        candidates.push(&seed.file.name, "seed", &seed.file.path);
    }
    for schema in &facts.schemas {
        for name in &schema.model_names {
            candidates.push(name, "schema model", &schema.path);
        }
        for seed in &schema.seeds {
            candidates.push(&seed.name, "schema seed", &schema.path);
        }
    }
    for file in &facts.sources {
        for entry in &file.entries {
            candidates.push(&entry.name, "source", &file.path);
        }
    }
    for test in &facts.tests {
        for block in &test.blocks {
            candidates.push(
                block_or_file_name(block.name.as_deref(), &test.file.name),
                "SQL test",
                &test.file.path,
            );
            for case in &block.cases {
                candidates.push(case, "SQL test case", &test.file.path);
            }
        }
    }
    candidates.push_named(&facts.scenarios, "scenario");
    for audit in &facts.audits {
        for block_name in &audit.block_names {
            candidates.push_audit(audit, block_name.as_deref());
        }
    }
    for file in &facts.declaration_files {
        candidates.push_declarations(&file.declarations, "declaration", file.private);
    }
    candidates.push_named(&facts.model_schemas, "model schema");
    candidates.push_named(&facts.materializations, "materialization");
    for kind in [NodeKind::Loader, NodeKind::Asset, NodeKind::Check] {
        for node in facts.nodes.iter().filter(|node| node.kind == kind) {
            candidates.push(&node.name, kind.label(), &node.path);
        }
    }
    candidates.push_named(&facts.hook_functions, "Python hook");
    candidates.push_named(&facts.event_exporters, "event exporter");
    candidates.push_named(&facts.providers, "provider");
    candidates.list
}

/// Python's `block.name or stem`.
pub(crate) fn block_or_file_name<'a>(block_name: Option<&'a str>, file_name: &'a str) -> &'a str {
    block_name
        .filter(|name| !name.is_empty())
        .unwrap_or(file_name)
}
