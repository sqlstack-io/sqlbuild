use crate::scope_index::_helpers::identities::{
    declaration_text, grant_through_text, resource_text,
};
use std::collections::HashMap;

use crate::scope_index::main::build_scope_index::build_scope_index;
use crate::scope_index::main::classify_resource::classify_resource;
use crate::scope_index::main::relationship_grants::relationship_grants;
use crate::scope_index::main::report_lookup::report_lookup;
use crate::scope_index::models::{
    ConsumerResource, DeclarationIdentity, DeclarationInput, DeclarationKind, GrantEntry,
    RelationshipFact, ReportLookup, ReportRecords, ResourceIdentity, ResourceInput, ResourceKind,
    ResourceRoot, ResourceVisibility, ScopeDeferral, ScopeIndex, ScopeInputs, ScopeKind,
    VisibilityTable,
};
use crate::scope_index::tests::test_types::{
    ClassifyTestCase, DeclarationRow, FactRow, GrantTestCase, IndexFactsTestCase, LookupTestCase,
    ResourceRow, VisibleRow,
};

const GLOBAL_ROOT_FALLBACKS: [(DeclarationKind, &str); 3] = [
    (DeclarationKind::Macro, "macros"),
    (DeclarationKind::Enum, "enums"),
    (DeclarationKind::Constant, "constants"),
];
const ROOTS: [(&str, ResourceRoot); 1] = [("seed", ResourceRoot::Seed)];

/// Scope inputs as the Python facade walks them for these resource and declaration rows.
pub(super) fn scope_inputs(
    resources: &[ResourceRow],
    declarations: &[DeclarationRow],
) -> ScopeInputs {
    ScopeInputs {
        resources: resources.iter().map(resource_input).collect(),
        declarations: declarations.iter().map(declaration_input).collect(),
    }
}

/// The native index of these rows, or its deferral.
pub(super) fn index_outcome(
    resources: &[ResourceRow],
    declarations: &[DeclarationRow],
) -> Result<ScopeIndex, ScopeDeferral> {
    build_scope_index(scope_inputs(resources, declarations))
}

/// The index of these rows, which must not defer.
pub(super) fn built_index(
    resources: &[ResourceRow],
    declarations: &[DeclarationRow],
) -> ScopeIndex {
    index_outcome(resources, declarations).expect("no deferral")
}

/// Qualified resource identities in `order`.
pub(super) fn resource_names(index: &ScopeIndex, order: &[usize]) -> Vec<String> {
    order
        .iter()
        .map(|resource| resource_text(&index.resources[*resource].identity))
        .collect()
}

/// `path:line` of the declarations in `order`.
pub(super) fn declaration_locations(index: &ScopeIndex, order: &[usize]) -> Vec<String> {
    order
        .iter()
        .map(|declaration| {
            let record = &index.declarations[*declaration];
            format!("{}:{}", record.path, record.line)
        })
        .collect()
}

/// `code message` of every diagnostic, in order.
pub(super) fn diagnostic_texts(index: &ScopeIndex) -> Vec<String> {
    index
        .diagnostics
        .iter()
        .map(|item| format!("{} {}", item.code.as_str(), item.message))
        .collect()
}

/// The index of a facts case, whose first declaration depends on the case's dependencies.
pub(super) fn index_with_dependencies(test_case: &IndexFactsTestCase) -> ScopeIndex {
    let mut inputs: ScopeInputs = scope_inputs(&[], test_case.declarations);
    inputs.declarations[0].dependencies = test_case
        .dependencies
        .iter()
        .map(|name| DeclarationIdentity {
            kind: DeclarationKind::Macro,
            name: (*name).to_owned(),
            owner: None,
        })
        .collect();
    build_scope_index(inputs).expect("no deferral")
}

/// `consumer -> dependency` text of every usage, in order.
pub(super) fn usage_texts(index: &ScopeIndex) -> Vec<String> {
    index
        .usages
        .iter()
        .map(|usage| {
            let consumer = &index.declarations[usage.consumer];
            format!(
                "{} -> {}",
                declaration_text(&consumer.identity),
                declaration_text(&consumer.dependencies[usage.dependency])
            )
        })
        .collect()
}

/// `resource declaration through kind` text of every grant a case's facts produce.
pub(super) fn grant_texts(test_case: &GrantTestCase) -> Vec<String> {
    let index: ScopeIndex = built_index(test_case.resources, test_case.declarations);
    let facts: Vec<RelationshipFact> = test_case.facts.iter().map(relationship_fact).collect();
    relationship_grants(&index, &facts)
        .expect("no deferral")
        .iter()
        .map(|grant| {
            format!(
                "{} {} {} {}",
                resource_text(&grant.resource),
                declaration_text(&index.declarations[grant.declaration].identity),
                grant_through_text(&index, grant),
                grant.kind.as_str()
            )
        })
        .collect()
}

/// The first resource identity of each resource group of a lookup case.
pub(super) fn resource_groups(test_case: &LookupTestCase) -> Vec<String> {
    let index: ScopeIndex = built_index(test_case.resources, test_case.declarations);
    let lookup: ReportLookup = report_lookup(&report_records(&index)).expect("valid paths");
    lookup
        .groups
        .resources
        .groups
        .iter()
        .map(|group| resource_text(&index.resources[group[0]].identity))
        .collect()
}

/// A built index's records as a full scope index's records.
fn report_records(index: &ScopeIndex) -> ReportRecords {
    ReportRecords {
        resources: index.resources.clone(),
        declarations: index.declarations.clone(),
        ..ReportRecords::default()
    }
}

/// The visibility position groups of a lookup case as `kind owner positions` text.
pub(super) fn visibility_texts(test_case: &LookupTestCase) -> Vec<String> {
    let index: ScopeIndex = built_index(test_case.resources, test_case.declarations);
    let visibility = report_lookup(&report_records(&index))
        .expect("valid paths")
        .visibility;
    let mut texts: Vec<String> = vec![format!("global {:?}", visibility.global)];
    texts.extend(
        visibility
            .private
            .iter()
            .map(|(owner, positions)| format!("private {} {positions:?}", resource_text(owner))),
    );
    texts.extend(
        visibility
            .local
            .iter()
            .map(|(owner, positions)| format!("local {owner} {positions:?}")),
    );
    texts.extend(
        visibility
            .inherited
            .iter()
            .map(|(owner, positions)| format!("inherited {owner} {positions:?}")),
    );
    texts
}

fn resource_input(row: &ResourceRow) -> ResourceInput {
    let (kind, name, path) = *row;
    ResourceInput {
        identity: resource(kind, name),
        path: path.to_owned(),
        root: ROOTS
            .iter()
            .find(|(root_kind, _root)| *root_kind == kind)
            .map_or(ResourceRoot::Kind, |(_kind, root)| *root),
    }
}

fn declaration_input(row: &DeclarationRow) -> DeclarationInput {
    let (kind, name, path, line, scope, owning_path) = *row;
    let scope: ScopeKind = ScopeKind::parse(scope).expect("scope kind");
    let kind: DeclarationKind = DeclarationKind::parse(kind).expect("declaration kind");
    let stem: &str = path.rsplit('/').next().unwrap_or(path);
    DeclarationInput {
        identity: DeclarationIdentity {
            kind,
            name: name.to_owned(),
            owner: (scope == ScopeKind::Private)
                .then(|| resource("model", stem.trim_end_matches(".sql"))),
        },
        path: path.to_owned(),
        line,
        scope,
        ownership_root: (scope != ScopeKind::Global).then(|| "models".to_owned()),
        root_fallback: GLOBAL_ROOT_FALLBACKS
            .iter()
            .find(|(fallback_kind, _root)| *fallback_kind == kind)
            .map_or("macros", |(_kind, root)| *root)
            .to_owned(),
        owning_path: owning_path.map(str::to_owned),
        dependencies: Vec::new(),
    }
}

fn relationship_fact(row: &FactRow) -> RelationshipFact {
    let (name, expected, called, tested) = *row;
    RelationshipFact {
        resource: resource("test", name),
        expected_models: owned(expected),
        called_macros: owned(called),
        tested_macros: owned(tested),
    }
}

fn resource(kind: &str, name: &str) -> ResourceIdentity {
    ResourceIdentity {
        kind: ResourceKind::parse(kind).expect("resource kind"),
        name: name.to_owned(),
    }
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

/// Six positions: global 0 and 4, local 2 under `sql/models/marts`, inherited 3 and 5.
fn consumer_table() -> VisibilityTable {
    VisibilityTable {
        identity_keys: vec![0, 1, 2, 3, 4, 5],
        global: vec![0, 4],
        local: HashMap::from([("sql/models/marts".to_owned(), vec![2])]),
        inherited: HashMap::from([
            ("models".to_owned(), vec![3]),
            ("sql/models/staging".to_owned(), vec![5]),
        ]),
    }
}

/// Classify one case's consumer against the shared table, or None when it defers.
pub(super) fn classified_rows(
    test_case: &ClassifyTestCase,
) -> Option<(Vec<VisibleRow>, Vec<usize>)> {
    let resource: ConsumerResource = ConsumerResource {
        private: test_case.private.to_vec(),
        path: test_case.path.to_owned(),
        grants: test_case
            .grants
            .iter()
            .map(|(identity_key, reason, through)| GrantEntry {
                identity_key: *identity_key,
                reason: *reason,
                through: *through,
            })
            .collect(),
    };
    let classified: ResourceVisibility = classify_resource(&consumer_table(), &resource).ok()?;
    Some((
        classified
            .visible
            .iter()
            .map(|entry| (entry.position, entry.reason.as_str(), entry.through))
            .collect(),
        classified.inaccessible,
    ))
}
