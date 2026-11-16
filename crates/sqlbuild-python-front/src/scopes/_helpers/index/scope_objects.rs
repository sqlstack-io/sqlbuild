//! Python `ScopeIndex` models read into the native full index, and built back from it.

use crate::scopes::models::ScopeClasses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyTuple;
use sqlbuild_scopes::placement::models::{PlacementIndex, PlacementUsage};
use sqlbuild_scopes::scope_cache::models::{
    CachedDeclaration, CachedDiagnostic, CachedInaccessible, CachedIndex, CachedUsage,
    CachedVisibility, ConstantFacts, EnumFacts, MacroFacts,
};
use sqlbuild_scopes::scope_index::models::{
    DeclarationIdentity, OwnershipRoot, QualifiedIdentity, ReportGrant, ResourceIdentity,
    ResourceKind, ResourceRecord,
};

use crate::scopes::_helpers::declarations::placement::{
    DeclarationRow, IdentityRow, QualifiedRow, RootRow, declaration_identity, declaration_record,
    ownership_root, qualified_identity, resource_identity,
};
use crate::scopes::_helpers::index::scope_lookup::grant_kind;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const ENUM_MODULE: &str = "enum";

/// `str(value.value)` of an enum, else `str(value)`.
pub(crate) fn enum_text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    let enum_type = value.py().import(ENUM_MODULE)?.getattr("Enum")?;
    if value.is_instance(&enum_type)? {
        return native_field(value, "value")?.str()?.extract();
    }
    value.str()?.extract()
}

pub(crate) fn value_of(item: &Bound<'_, PyAny>, name: &str) -> PyResult<String> {
    native_field(&native_field(item, name)?, "value")?.extract()
}

pub(crate) fn field<'py, T: pyo3::FromPyObject<'py>>(
    item: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<T> {
    native_field(item, name)?.extract()
}

pub(crate) fn optional_item<'py>(
    item: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let value: Bound<'py, PyAny> = native_field(item, name)?;
    Ok(if value.is_none() { None } else { Some(value) })
}

fn items<'py>(item: &Bound<'py, PyAny>, name: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    native_field(item, name)?.try_iter()?.collect()
}

/// A resource identity's `(kind, name)` row.
pub(crate) fn identity_row(identity: &Bound<'_, PyAny>) -> PyResult<IdentityRow> {
    Ok((value_of(identity, "kind")?, field(identity, "name")?))
}

/// Python's `qualified_row` of a resource or declaration identity.
pub(crate) fn qualified_row(identity: &Bound<'_, PyAny>) -> PyResult<QualifiedRow> {
    let owner: Option<IdentityRow> = match identity.getattr_opt("owner")? {
        Some(owner) if !owner.is_none() => Some(identity_row(&owner)?),
        _ => None,
    };
    Ok((value_of(identity, "kind")?, field(identity, "name")?, owner))
}

/// A resource or declaration identity as the native qualified identity.
pub(crate) fn read_qualified(identity: &Bound<'_, PyAny>) -> PyResult<QualifiedIdentity> {
    qualified_identity(qualified_row(identity)?)
}

fn read_resource_identity(identity: &Bound<'_, PyAny>) -> PyResult<ResourceIdentity> {
    resource_identity(identity_row(identity)?)
}

pub(crate) fn read_declaration_identity(
    identity: &Bound<'_, PyAny>,
) -> PyResult<DeclarationIdentity> {
    declaration_identity(qualified_row(identity)?)
}

fn read_optional_qualified(
    item: &Bound<'_, PyAny>,
    name: &str,
) -> PyResult<Option<QualifiedIdentity>> {
    optional_item(item, name)?
        .map(|value| read_qualified(&value))
        .transpose()
}

fn root_row(root: &Bound<'_, PyAny>) -> PyResult<RootRow> {
    let resource_kind: Option<String> = optional_item(root, "resource_kind")?
        .map(|kind| native_field(&kind, "value")?.extract())
        .transpose()?;
    Ok((field(root, "path")?, value_of(root, "kind")?, resource_kind))
}

fn read_root(root: &Bound<'_, PyAny>) -> PyResult<OwnershipRoot> {
    ownership_root(root_row(root)?)
}

/// One `ResourceRecord` as its native record.
pub(crate) fn read_resource(record: &Bound<'_, PyAny>) -> PyResult<ResourceRecord> {
    Ok(ResourceRecord {
        identity: read_resource_identity(&native_field(record, "identity")?)?,
        path: field(record, "path")?,
        ownership_root: read_root(&native_field(record, "ownership_root")?)?,
    })
}

fn read_macro(facts: &Bound<'_, PyAny>) -> PyResult<MacroFacts> {
    Ok(MacroFacts {
        parameters: field(facts, "parameters")?,
        dependencies: items(facts, "dependencies")?
            .iter()
            .map(read_declaration_identity)
            .collect::<PyResult<_>>()?,
        source_digest: field(facts, "source_digest")?,
    })
}

fn read_enum(facts: &Bound<'_, PyAny>) -> PyResult<EnumFacts> {
    Ok(EnumFacts {
        members: items(facts, "members")?
            .iter()
            .map(|member| field(member, "name"))
            .collect::<PyResult<_>>()?,
        scalar_type: field(facts, "scalar_type")?,
    })
}

fn read_constant(facts: &Bound<'_, PyAny>) -> PyResult<ConstantFacts> {
    Ok(ConstantFacts {
        logical_type: field(facts, "logical_type")?,
        collection_kind: field(facts, "collection_kind")?,
        item_count: field(facts, "item_count")?,
        nullable: field(facts, "nullable")?,
        render_as: field(facts, "render_as")?,
    })
}

/// One `DeclarationRecord` with its value metadata.
pub(crate) fn read_declaration(record: &Bound<'_, PyAny>) -> PyResult<CachedDeclaration> {
    Ok(CachedDeclaration {
        record: declaration_record(DeclarationRow {
            identity: qualified_row(&native_field(record, "identity")?)?,
            path: field(record, "path")?,
            line: field(record, "line")?,
            column: field(record, "column")?,
            scope: value_of(record, "scope")?,
            root: root_row(&native_field(record, "ownership_root")?)?,
            owning_path: field(record, "owning_path")?,
        })?,
        macro_facts: optional_item(record, "macro")?
            .map(|facts| read_macro(&facts))
            .transpose()?,
        enum_facts: optional_item(record, "enum")?
            .map(|facts| read_enum(&facts))
            .transpose()?,
        constant_facts: optional_item(record, "constant")?
            .map(|facts| read_constant(&facts))
            .transpose()?,
    })
}

fn read_usage(usage: &Bound<'_, PyAny>) -> PyResult<CachedUsage> {
    Ok(CachedUsage {
        consumer: read_qualified(&native_field(usage, "consumer")?)?,
        declaration: read_declaration_identity(&native_field(usage, "declaration")?)?,
        kind: value_of(usage, "kind")?,
        through: read_optional_qualified(usage, "through")?,
        enum_member: field(usage, "enum_member")?,
    })
}

fn read_grant(grant: &Bound<'_, PyAny>) -> PyResult<ReportGrant> {
    Ok(ReportGrant {
        resource: read_resource_identity(&native_field(grant, "resource")?)?,
        declaration: read_declaration_identity(&native_field(grant, "declaration")?)?,
        through: read_qualified(&native_field(grant, "through")?)?,
        kind: grant_kind(&value_of(grant, "kind")?)?,
    })
}

fn read_visibility(fact: &Bound<'_, PyAny>) -> PyResult<CachedVisibility> {
    Ok(CachedVisibility {
        resource: read_resource_identity(&native_field(fact, "resource")?)?,
        declaration: read_declaration_identity(&native_field(fact, "declaration")?)?,
        reason: value_of(fact, "reason")?,
        through: read_optional_qualified(fact, "through")?,
    })
}

fn read_inaccessible(fact: &Bound<'_, PyAny>) -> PyResult<CachedInaccessible> {
    Ok(CachedInaccessible {
        resource: read_resource_identity(&native_field(fact, "resource")?)?,
        declaration: read_declaration_identity(&native_field(fact, "declaration")?)?,
        reason: value_of(fact, "reason")?,
    })
}

/// One `ScopeDiagnostic`, with its code and severity as their text.
pub(crate) fn read_diagnostic(diagnostic: &Bound<'_, PyAny>) -> PyResult<CachedDiagnostic> {
    Ok(CachedDiagnostic {
        code: enum_text(&native_field(diagnostic, "code")?)?,
        message: field(diagnostic, "message")?,
        severity: enum_text(&native_field(diagnostic, "severity")?)?,
        path: field(diagnostic, "path")?,
        line: field(diagnostic, "line")?,
        column: field(diagnostic, "column")?,
        declaration: optional_item(diagnostic, "declaration")?
            .map(|identity| read_declaration_identity(&identity))
            .transpose()?,
        resource: optional_item(diagnostic, "resource")?
            .map(|identity| read_resource_identity(&identity))
            .transpose()?,
    })
}

/// The six `ScopeCompleteness` sections in field order.
pub(crate) fn read_completeness(index: &Bound<'_, PyAny>) -> PyResult<[bool; 6]> {
    let done: Bound<'_, PyAny> = native_field(index, "completeness")?;
    Ok([
        field(&done, "discovery")?,
        field(&done, "static_visibility")?,
        field(&done, "runtime_usage")?,
        field(&done, "relationships")?,
        field(&done, "placement")?,
        field(&done, "promotion_impact")?,
    ])
}

fn read_all<T>(
    index: &Bound<'_, PyAny>,
    name: &str,
    read: impl Fn(&Bound<'_, PyAny>) -> PyResult<T>,
) -> PyResult<Vec<T>> {
    items(index, name)?.iter().map(read).collect()
}

/// Every record of a Python `ScopeIndex`, in its tuple orders.
pub(crate) fn read_index(index: &Bound<'_, PyAny>) -> PyResult<CachedIndex> {
    Ok(CachedIndex {
        roots: read_all(index, "ownership_roots", read_root)?,
        resources: read_all(index, "resources", read_resource)?,
        declarations: read_all(index, "declarations", read_declaration)?,
        usages: read_all(index, "usages", read_usage)?,
        grants: read_all(index, "grants", read_grant)?,
        visibility: read_all(index, "visibility", read_visibility)?,
        inaccessible: read_all(index, "inaccessible", read_inaccessible)?,
        diagnostics: read_all(index, "diagnostics", read_diagnostic)?,
        completeness: read_completeness(index)?,
    })
}

/// A Python `ScopeIndex`'s resources, declarations and usages for the placement rules.
pub(crate) fn read_placement_index(index: &Bound<'_, PyAny>) -> PyResult<PlacementIndex> {
    Ok(PlacementIndex {
        resources: read_all(index, "resources", read_resource)?,
        declarations: read_all(index, "declarations", |record| {
            Ok(read_declaration(record)?.record)
        })?,
        usages: read_all(index, "usages", |usage| {
            Ok(PlacementUsage {
                consumer: read_qualified(&native_field(usage, "consumer")?)?,
                declaration: read_declaration_identity(&native_field(usage, "declaration")?)?,
                through: read_optional_qualified(usage, "through")?,
            })
        })?,
    })
}

/// A `ResourceIdentity`.
pub(crate) fn resource_identity_object<'py>(
    classes: &ScopeClasses<'py>,
    identity: &ResourceIdentity,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::ResourceIdentity)?.call1((
        classes.member(RecordId::ResourceKind, identity.kind.as_str())?,
        identity.name.as_str(),
    ))
}

/// A `DeclarationIdentity`.
pub(crate) fn declaration_identity_object<'py>(
    classes: &ScopeClasses<'py>,
    identity: &DeclarationIdentity,
) -> PyResult<Bound<'py, PyAny>> {
    let owner: Option<Bound<'py, PyAny>> = identity
        .owner
        .as_ref()
        .map(|owner| resource_identity_object(classes, owner))
        .transpose()?;
    classes.model(RecordId::DeclarationIdentity)?.call1((
        classes.member(RecordId::DeclarationKind, identity.kind.as_str())?,
        identity.name.as_str(),
        owner,
    ))
}

/// A `ResourceIdentity` or `DeclarationIdentity`.
pub(crate) fn qualified_object<'py>(
    classes: &ScopeClasses<'py>,
    identity: &QualifiedIdentity,
) -> PyResult<Bound<'py, PyAny>> {
    match identity {
        QualifiedIdentity::Resource(identity) => resource_identity_object(classes, identity),
        QualifiedIdentity::Declaration(identity) => declaration_identity_object(classes, identity),
    }
}

fn optional_qualified_object<'py>(
    classes: &ScopeClasses<'py>,
    identity: Option<&QualifiedIdentity>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    identity
        .map(|identity| qualified_object(classes, identity))
        .transpose()
}

/// An `OwnershipRoot`.
pub(crate) fn root_object<'py>(
    classes: &ScopeClasses<'py>,
    root: &OwnershipRoot,
) -> PyResult<Bound<'py, PyAny>> {
    let resource_kind: Option<Bound<'py, PyAny>> = root
        .resource_kind
        .map(|kind| classes.member(RecordId::ResourceKind, ResourceKind::as_str(kind)))
        .transpose()?;
    classes.model(RecordId::OwnershipRoot)?.call1((
        root.path.as_str(),
        classes.member(RecordId::OwnershipRootKind, root.kind.as_str())?,
        resource_kind,
    ))
}

fn resource_object<'py>(
    classes: &ScopeClasses<'py>,
    record: &ResourceRecord,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::ResourceRecord)?.call1((
        resource_identity_object(classes, &record.identity)?,
        record.path.as_str(),
        root_object(classes, &record.ownership_root)?,
    ))
}

fn tuple_of<'py, T>(
    py: Python<'py>,
    values: &[T],
    build: impl Fn(&T) -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let built: Vec<Bound<'py, PyAny>> = values.iter().map(build).collect::<PyResult<_>>()?;
    PyTuple::new(py, built)
}

fn macro_object<'py>(
    classes: &ScopeClasses<'py>,
    facts: &MacroFacts,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    classes.model(RecordId::MacroMetadata)?.call1((
        PyTuple::new(py, &facts.parameters)?,
        tuple_of(py, &facts.dependencies, |identity| {
            declaration_identity_object(classes, identity)
        })?,
        facts.source_digest.as_str(),
    ))
}

fn enum_object<'py>(classes: &ScopeClasses<'py>, facts: &EnumFacts) -> PyResult<Bound<'py, PyAny>> {
    let member_class: Bound<'py, PyAny> = classes.model(RecordId::EnumMemberMetadata)?;
    classes.model(RecordId::EnumMetadata)?.call1((
        tuple_of(classes.py(), &facts.members, |name| {
            member_class.call1((name.as_str(),))
        })?,
        facts.scalar_type.as_str(),
    ))
}

fn constant_object<'py>(
    classes: &ScopeClasses<'py>,
    facts: &ConstantFacts,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::ConstantMetadata)?.call1((
        facts.logical_type.as_str(),
        facts.collection_kind.as_deref(),
        facts.item_count,
        facts.nullable,
        facts.render_as.as_deref(),
    ))
}

fn declaration_object<'py>(
    classes: &ScopeClasses<'py>,
    item: &CachedDeclaration,
) -> PyResult<Bound<'py, PyAny>> {
    let record = &item.record;
    let values: [Option<Bound<'py, PyAny>>; 3] = [
        item.macro_facts
            .as_ref()
            .map(|facts| macro_object(classes, facts))
            .transpose()?,
        item.enum_facts
            .as_ref()
            .map(|facts| enum_object(classes, facts))
            .transpose()?,
        item.constant_facts
            .as_ref()
            .map(|facts| constant_object(classes, facts))
            .transpose()?,
    ];
    let [macro_metadata, enum_metadata, constant_metadata] = values;
    classes.model(RecordId::DeclarationRecord)?.call1((
        declaration_identity_object(classes, &record.identity)?,
        record.path.as_str(),
        record.line,
        record.column,
        classes.member(RecordId::ScopeKind, record.scope.as_str())?,
        root_object(classes, &record.ownership_root)?,
        record.owning_path.as_deref(),
        macro_metadata,
        enum_metadata,
        constant_metadata,
    ))
}

fn usage_object<'py>(
    classes: &ScopeClasses<'py>,
    item: &CachedUsage,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::UsageRecord)?.call1((
        qualified_object(classes, &item.consumer)?,
        declaration_identity_object(classes, &item.declaration)?,
        classes.member(RecordId::UsageKind, &item.kind)?,
        optional_qualified_object(classes, item.through.as_ref())?,
        item.enum_member.as_deref(),
    ))
}

fn grant_object<'py>(
    classes: &ScopeClasses<'py>,
    item: &ReportGrant,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::GrantRecord)?.call1((
        resource_identity_object(classes, &item.resource)?,
        declaration_identity_object(classes, &item.declaration)?,
        qualified_object(classes, &item.through)?,
        classes.member(RecordId::GrantKind, item.kind.as_str())?,
    ))
}

fn visibility_object<'py>(
    classes: &ScopeClasses<'py>,
    item: &CachedVisibility,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::VisibilityRecord)?.call1((
        resource_identity_object(classes, &item.resource)?,
        declaration_identity_object(classes, &item.declaration)?,
        classes.member(RecordId::VisibilityReason, &item.reason)?,
        optional_qualified_object(classes, item.through.as_ref())?,
    ))
}

fn inaccessible_object<'py>(
    classes: &ScopeClasses<'py>,
    item: &CachedInaccessible,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::InaccessibleRecord)?.call1((
        resource_identity_object(classes, &item.resource)?,
        declaration_identity_object(classes, &item.declaration)?,
        classes.member(RecordId::InaccessibleReason, &item.reason)?,
    ))
}

/// A `ScopeDiagnostic`.
pub(crate) fn diagnostic_object<'py>(
    classes: &ScopeClasses<'py>,
    item: &CachedDiagnostic,
) -> PyResult<Bound<'py, PyAny>> {
    classes.model(RecordId::ScopeDiagnostic)?.call1((
        classes.member(RecordId::ScopeDiagnosticCode, &item.code)?,
        item.message.as_str(),
        classes.member(RecordId::ScopeDiagnosticSeverity, &item.severity)?,
        item.path.as_deref(),
        item.line,
        item.column,
        item.declaration
            .as_ref()
            .map(|identity| declaration_identity_object(classes, identity))
            .transpose()?,
        item.resource
            .as_ref()
            .map(|identity| resource_identity_object(classes, identity))
            .transpose()?,
    ))
}

/// A `ScopeCompleteness`.
pub(crate) fn completeness_object<'py>(
    classes: &ScopeClasses<'py>,
    done: [bool; 6],
) -> PyResult<Bound<'py, PyAny>> {
    classes
        .model(RecordId::ScopeCompleteness)?
        .call1((done[0], done[1], done[2], done[3], done[4], done[5]))
}

/// A Python `ScopeIndex` of a full native index.
pub(crate) fn index_object<'py>(
    classes: &ScopeClasses<'py>,
    index: &CachedIndex,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    classes.model(RecordId::ScopeIndex)?.call1((
        tuple_of(py, &index.roots, |root| root_object(classes, root))?,
        tuple_of(py, &index.resources, |record| {
            resource_object(classes, record)
        })?,
        tuple_of(py, &index.declarations, |item| {
            declaration_object(classes, item)
        })?,
        tuple_of(py, &index.usages, |item| usage_object(classes, item))?,
        tuple_of(py, &index.grants, |item| grant_object(classes, item))?,
        tuple_of(py, &index.visibility, |item| {
            visibility_object(classes, item)
        })?,
        tuple_of(py, &index.inaccessible, |item| {
            inaccessible_object(classes, item)
        })?,
        tuple_of(py, &index.diagnostics, |item| {
            diagnostic_object(classes, item)
        })?,
        completeness_object(classes, index.completeness)?,
    ))
}

/// A tuple attribute of a scope model.
pub(crate) fn tuple_attribute<'py>(
    item: &Bound<'py, PyAny>,
    name: &str,
) -> PyResult<Bound<'py, PyTuple>> {
    Ok(native_field(item, name)?.downcast_into::<PyTuple>()?)
}
