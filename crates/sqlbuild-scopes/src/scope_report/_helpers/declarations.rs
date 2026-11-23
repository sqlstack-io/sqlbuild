//! `declaration_report`: one declaration's value-safe report, with its role container.

use std::collections::BTreeSet;

use sqlbuild_core::json::models::{JsonInteger, JsonValue};

use crate::scope_index::main::format_identity::format_identity;
use crate::scope_index::models::{
    DeclarationKind, DeclarationRecord, QualifiedIdentity, ScopeKind,
};
use crate::scope_report::_helpers::context::Context;
use crate::scope_report::_helpers::paths::safe_scope_path;
use crate::scope_index::constants::GLOBAL_ROLE_PARENT;
use crate::scope_report::constants::{ENUM_MEMBER_PREVIEW, GROUP_DIRECTORY, PATH_SEPARATOR};
use crate::scope_report::models::{
    DeclarationReport, DeclarationValues, MacroValues, Provenance, SourceLocation,
};

/// The kind folders of a macro role, which are layout rather than buckets.
const MACRO_KIND_DIRECTORIES: [&str; 2] = ["python", "sql"];

/// `_declaration_container`: role, visibility label, role root and bucket path.
pub(crate) struct Container {
    pub(crate) role: Option<String>,
    pub(crate) visibility: &'static str,
    pub(crate) role_root: Option<String>,
    pub(crate) bucket: Option<String>,
}

pub(crate) fn declaration_text(record: &DeclarationRecord) -> String {
    format_identity(&QualifiedIdentity::Declaration(record.identity.clone()))
}

pub(crate) fn text(value: &str) -> JsonValue {
    JsonValue::String(value.to_owned())
}

pub(crate) fn optional_text(value: Option<&str>) -> JsonValue {
    value.map_or(JsonValue::Null, text)
}

pub(crate) fn integer(value: i64) -> JsonValue {
    JsonValue::Integer(JsonInteger::from(value))
}

pub(crate) fn texts(values: &[String]) -> JsonValue {
    JsonValue::Array(values.iter().map(|value| text(value)).collect())
}

/// Python's visibility label of each scope.
fn visibility_label(scope: ScopeKind) -> &'static str {
    match scope {
        ScopeKind::Global => "project",
        ScopeKind::Inherited => "descendant_public",
        ScopeKind::Local => "exact_owner_private",
        ScopeKind::Private => "model_private",
    }
}

/// `_declaration_container` of one record.
pub(crate) fn container(record: &DeclarationRecord) -> Container {
    let visibility: &'static str = visibility_label(record.scope);
    if record.scope == ScopeKind::Private {
        return Container {
            role: None,
            visibility,
            role_root: None,
            bucket: None,
        };
    }
    let role_parts: &[&str] = record.identity.kind.role_parts();
    let safe: String = safe_scope_path(&record.path);
    let parts: Vec<&str> = safe.split(PATH_SEPARATOR).collect();
    let mut role_index: usize = match record.owning_path.as_deref() {
        Some(owning) if !owning.is_empty() => owning.split(PATH_SEPARATOR).count(),
        _ => usize::from(parts.first() == Some(&GLOBAL_ROLE_PARENT)),
    };
    if parts.get(role_index) == Some(&GROUP_DIRECTORY) {
        role_index += 1;
    }
    let role_end: usize = (role_index + role_parts.len()).min(parts.len());
    let bucket_end: usize = parts.len().saturating_sub(1).max(role_end);
    let bucket_start: usize = if record.identity.kind == DeclarationKind::Macro
        && role_end < bucket_end
        && MACRO_KIND_DIRECTORIES.contains(&parts[role_end])
    {
        role_end + 1
    } else {
        role_end
    };
    let bucket: String = parts[bucket_start..bucket_end].join("/");
    Container {
        role: Some(role_parts.join("/")),
        visibility,
        role_root: Some(parts[..role_end].join("/")),
        bucket: (!bucket.is_empty()).then_some(bucket),
    }
}

/// The report metadata of one declaration's values and container.
fn metadata(values: Option<&DeclarationValues>, container: Container) -> Vec<(String, JsonValue)> {
    let mut metadata: Vec<(String, JsonValue)> = Vec::new();
    let mut push = |key: &str, value: JsonValue| metadata.push((key.to_owned(), value));
    if let Some(values) = values {
        if let Some(macro_values) = &values.macro_values {
            push("parameters", texts(&macro_values.parameters));
            push(
                "dependency_count",
                integer(i64::try_from(macro_values.dependencies.len()).unwrap_or(i64::MAX)),
            );
        }
        if let Some(enum_values) = &values.enum_values {
            let shown: usize = enum_values.members.len().min(ENUM_MEMBER_PREVIEW);
            push("scalar_type", text(&enum_values.scalar_type));
            push("members", texts(&enum_values.members[..shown]));
            push(
                "member_count",
                integer(i64::try_from(enum_values.members.len()).unwrap_or(i64::MAX)),
            );
            push(
                "members_truncated",
                JsonValue::Bool(enum_values.members.len() > ENUM_MEMBER_PREVIEW),
            );
        }
        if let Some(constant) = &values.constant_values {
            push("logical_type", text(&constant.logical_type));
            push(
                "collection_kind",
                optional_text(constant.collection_kind.as_deref()),
            );
            push(
                "item_count",
                constant.item_count.map_or(JsonValue::Null, integer),
            );
            push("nullable", JsonValue::Bool(constant.nullable));
            push("render_as", optional_text(constant.render_as.as_deref()));
        }
    }
    let Container {
        role,
        visibility,
        role_root,
        bucket,
    } = container;
    push("role", optional_text(role.as_deref()));
    push("visibility_label", text(visibility));
    push("role_root", optional_text(role_root.as_deref()));
    push("bucket_path", optional_text(bucket.as_deref()));
    metadata
}

/// `required_placement`: the scope, folder and promotion impact consumers require.
fn required(
    context: &Context<'_>,
    record: &DeclarationRecord,
) -> (Option<String>, Option<String>, Vec<String>) {
    if context.usages_of_declaration(&record.identity).is_empty()
        || !context.completeness().runtime_usage
    {
        return (None, None, Vec::new());
    }
    let Some(placement) = context.required_placement(&record.identity) else {
        return (None, None, Vec::new());
    };
    let current: &str = record
        .owning_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .unwrap_or(&record.ownership_root.path);
    let promotion: Vec<String> =
        if placement.scope != record.scope || placement.path.as_deref() != Some(current) {
            placement.consumers.clone()
        } else {
            Vec::new()
        };
    (
        Some(placement.scope.as_str().to_owned()),
        placement.path.clone(),
        promotion,
    )
}

/// The qualified identity text of each macro dependency.
fn dependency_texts(macro_values: &MacroValues) -> Vec<String> {
    macro_values
        .dependencies
        .iter()
        .map(|identity| format_identity(&QualifiedIdentity::Declaration(identity.clone())))
        .collect()
}

/// `declaration_report` of the record at input `position`.
pub(crate) fn declaration_report(
    context: &Context<'_>,
    position: usize,
    visibility: Option<Provenance>,
    inaccessible_reason: Option<String>,
) -> DeclarationReport {
    let record: &DeclarationRecord = context.declaration(position);
    let values: Option<&DeclarationValues> = context.values(position);
    let consumers: BTreeSet<String> = context
        .usages_of_declaration(&record.identity)
        .iter()
        .map(|usage| format_identity(&context.usage(*usage).consumer))
        .collect();
    let mut dependencies: Vec<String> = values
        .and_then(|values| values.macro_values.as_ref())
        .map(dependency_texts)
        .unwrap_or_default();
    dependencies.sort();
    let grants: BTreeSet<String> = context
        .grants_of_declaration(&record.identity)
        .iter()
        .map(|grant| {
            format_identity(&QualifiedIdentity::Resource(
                context.grant(*grant).resource.clone(),
            ))
        })
        .collect();
    let (required_scope, required_path, promotion_impact) = required(context, record);
    DeclarationReport {
        identity: declaration_text(record),
        kind: record.identity.kind.as_str().to_owned(),
        name: record.identity.name.clone(),
        owner: record
            .identity
            .owner
            .as_ref()
            .map(|owner| format_identity(&QualifiedIdentity::Resource(owner.clone()))),
        definition: SourceLocation {
            path: safe_scope_path(&record.path),
            line: record.line,
            column: record.column,
        },
        scope: record.scope.as_str().to_owned(),
        owning_path: record.owning_path.clone(),
        visibility,
        inaccessible_reason,
        metadata: metadata(values, container(record)),
        consumers: consumers.into_iter().collect(),
        dependencies,
        grants: grants.into_iter().collect(),
        required_scope,
        required_path,
        promotion_impact,
    }
}
