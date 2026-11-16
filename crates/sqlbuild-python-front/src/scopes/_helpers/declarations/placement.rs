//! Scope identity, root and record rows as the native scope records.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;
use sqlbuild_scopes::scope_index::models::{
    DeclarationIdentity, DeclarationKind, DeclarationRecord, OwnershipRoot, OwnershipRootKind,
    QualifiedIdentity, ResourceIdentity, ResourceKind, ScopeKind,
};

pub(crate) type IdentityRow = (String, String);
/// A declaration's kind, name and private owner; a resource's kind and name with no owner.
pub(crate) type QualifiedRow = (String, String, Option<IdentityRow>);
/// An ownership root's path, kind and resource kind.
pub(crate) type RootRow = (String, String, Option<String>);
/// Identity, path, line, column, scope, ownership root and owning path.
pub(crate) struct DeclarationRow {
    pub(crate) identity: QualifiedRow,
    pub(crate) path: String,
    pub(crate) line: i64,
    pub(crate) column: i64,
    pub(crate) scope: String,
    pub(crate) root: RootRow,
    pub(crate) owning_path: Option<String>,
}

fn invalid(what: &str) -> pyo3::PyErr {
    PyValueError::new_err(format!("invalid scope placement {what}"))
}

pub(crate) fn resource_identity((kind, name): IdentityRow) -> PyResult<ResourceIdentity> {
    Ok(ResourceIdentity {
        kind: ResourceKind::parse(&kind).ok_or_else(|| invalid("resource kind"))?,
        name,
    })
}

pub(crate) fn declaration_identity(
    (kind, name, owner): QualifiedRow,
) -> PyResult<DeclarationIdentity> {
    Ok(DeclarationIdentity {
        kind: DeclarationKind::parse(&kind).ok_or_else(|| invalid("declaration kind"))?,
        name,
        owner: owner.map(resource_identity).transpose()?,
    })
}

/// Resource and declaration kinds share no values, so the kind tells the identities apart.
pub(crate) fn qualified_identity(row: QualifiedRow) -> PyResult<QualifiedIdentity> {
    if row.2.is_none()
        && let Some(kind) = ResourceKind::parse(&row.0)
    {
        return Ok(QualifiedIdentity::Resource(ResourceIdentity {
            kind,
            name: row.1,
        }));
    }
    Ok(QualifiedIdentity::Declaration(declaration_identity(row)?))
}

pub(crate) fn ownership_root((path, kind, resource_kind): RootRow) -> PyResult<OwnershipRoot> {
    Ok(OwnershipRoot {
        path,
        kind: OwnershipRootKind::parse(&kind).ok_or_else(|| invalid("root kind"))?,
        resource_kind: resource_kind
            .map(|kind| ResourceKind::parse(&kind).ok_or_else(|| invalid("resource kind")))
            .transpose()?,
    })
}

pub(crate) fn scope_kind(value: &str) -> PyResult<ScopeKind> {
    ScopeKind::parse(value).ok_or_else(|| invalid("scope"))
}

/// A declaration row as its record, without macro dependencies.
pub(crate) fn declaration_record(row: DeclarationRow) -> PyResult<DeclarationRecord> {
    let DeclarationRow {
        identity,
        path,
        line,
        column,
        scope,
        root,
        owning_path,
    } = row;
    Ok(DeclarationRecord {
        identity: declaration_identity(identity)?,
        path,
        line,
        column,
        scope: scope_kind(&scope)?,
        ownership_root: ownership_root(root)?,
        owning_path,
        dependencies: Vec::new(),
    })
}
