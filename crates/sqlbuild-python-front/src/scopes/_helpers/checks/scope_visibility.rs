//! Static declaration visibility of scope targets and authored paths over a `ScopeLookup`.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use pyo3::{IntoPyObjectExt, pyfunction};
use sqlbuild_scopes::scope_index::main::normalize_scope_path::normalize_scope_path;
use sqlbuild_scopes::scope_index::main::rejected_path_message::rejected_path_message;
use sqlbuild_scopes::scope_index::models::{
    OwnershipRoot, OwnershipRootKind, RejectedPath, ResourceIdentity, ResourceKind, ResourceRecord,
};

use crate::scopes::_helpers::index::scope_lookup::{
    NativeScopeLookup, ResolvedRows, TargetRows, scope_path_error,
};
use crate::scopes::_helpers::index::scope_objects::{
    qualified_row, read_resource, tuple_attribute,
};
use crate::scopes::models::ScopeClasses;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

type PyObject = Py<PyAny>;

const CURRENT_PATH: &str = ".";

fn native_lookup(lookup: &Bound<'_, PyAny>) -> PyResult<Py<NativeScopeLookup>> {
    native_field(lookup, "native")?.extract()
}

/// Python's `query_target`: the resources or declarations `target` names.
fn target_query<'py>(
    classes: &ScopeClasses<'py>,
    lookup: &Bound<'py, PyAny>,
    target: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    let identity: bool = target.is_instance(&classes.model(RecordId::ResourceIdentity)?)?
        || target.is_instance(&classes.model(RecordId::DeclarationIdentity)?)?;
    let raw: String = if identity {
        String::new()
    } else {
        target.str()?.extract()?
    };
    let row = if identity {
        Some(qualified_row(target)?)
    } else {
        None
    };
    let TargetRows {
        declarations,
        positions,
        identity_error,
        path_error,
    }: TargetRows = native_lookup(lookup)?.get().target(row, &raw)?;
    if let Some(message) = identity_error {
        return Err(PyErr::from_value(
            error_class(py, ErrorId::InvalidQualifiedIdentity)?.call1((message,))?,
        ));
    }
    if let Some(message) = path_error {
        return Err(scope_path_error(py, message)?);
    }
    let value: Bound<'py, PyAny> = if identity {
        target.clone()
    } else {
        raw.into_bound_py_any(py)?
    };
    let index: Bound<'py, PyAny> = native_field(lookup, "index")?;
    let records: Bound<'py, PyTuple> = tuple_attribute(
        &index,
        if declarations {
            "declarations"
        } else {
            "resources"
        },
    )?;
    let matched: Vec<Bound<'py, PyAny>> = positions
        .iter()
        .map(|position| records.get_item(*position))
        .collect::<PyResult<_>>()?;
    let matched: Bound<'py, PyTuple> = PyTuple::new(py, matched)?;
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item(
        if declarations {
            "declaration_matches"
        } else {
            "matches"
        },
        matched,
    )?;
    classes
        .model(RecordId::ScopeTargetQuery)?
        .call((value,), Some(&keywords))
}

/// `query_target`: the `ScopeTargetQuery` of `target`; invalid text raises the scope error.
#[pyfunction]
#[pyo3(name = "query_scope_target", signature = (*, lookup, target))]
pub(crate) fn scope_query_target(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    target: &Bound<'_, PyAny>,
) -> PyResult<PyObject> {
    Ok(target_query(&ScopeClasses::new(py)?, lookup, target)?.unbind())
}

fn resolved(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    resource: &ResourceRecord,
) -> PyResult<ResolvedRows> {
    let rows: ResolvedRows = native_lookup(lookup)?.get().resolve(py, resource)?;
    if let Some(message) = rows.2 {
        return Err(scope_path_error(py, message)?);
    }
    Ok(rows)
}

/// Python's `resolve_scope_visibility`: visible facts and inaccessible reasons per target.
#[pyfunction]
#[pyo3(signature = (*, lookup, target))]
pub(crate) fn resolve_scope_visibility(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    target: &Bound<'_, PyAny>,
) -> PyResult<PyObject> {
    resolved_visibility(py, lookup, target, true)
}

/// Python's `resolve_scope_declaration_visibility`: visible facts and inaccessible identities.
#[pyfunction]
#[pyo3(signature = (*, lookup, target))]
pub(crate) fn resolve_scope_declaration_visibility(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    target: &Bound<'_, PyAny>,
) -> PyResult<PyObject> {
    resolved_visibility(py, lookup, target, false)
}

/// Every visible fact of each matched resource, and its inaccessible declarations.
fn resolved_visibility(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    target: &Bound<'_, PyAny>,
    with_reasons: bool,
) -> PyResult<PyObject> {
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let query: Bound<'_, PyAny> = target_query(&classes, lookup, target)?;
    let index: Bound<'_, PyAny> = native_field(lookup, "index")?;
    let identities: Bound<'_, PyTuple> =
        tuple_attribute(&native_field(lookup, "visibility_index")?, "identities")?;
    let declarations: Bound<'_, PyTuple> = tuple_attribute(&index, "declarations")?;
    let grants: Bound<'_, PyTuple> = tuple_attribute(&index, "grants")?;
    let visible_class: Bound<'_, PyAny> = classes.model(RecordId::VisibilityRecord)?;
    let inaccessible_class: Bound<'_, PyAny> = classes.model(RecordId::InaccessibleRecord)?;
    let mut visible: Vec<Bound<'_, PyAny>> = Vec::new();
    let mut inaccessible: Vec<Bound<'_, PyAny>> = Vec::new();
    for resource in tuple_attribute(&query, "matches")?.iter() {
        let (resource_visible, resource_inaccessible, _) =
            resolved(py, lookup, &read_resource(&resource)?)?;
        let owner: Bound<'_, PyAny> = native_field(&resource, "identity")?;
        for (position, reason, grant) in resource_visible {
            let through: Option<Bound<'_, PyAny>> = grant
                .map(|grant| native_field(&grants.get_item(grant)?, "through"))
                .transpose()?;
            visible.push(visible_class.call1((
                &owner,
                identities.get_item(position)?,
                classes.member(RecordId::VisibilityReason, reason)?,
                through,
            ))?);
        }
        for (position, reason) in resource_inaccessible {
            let identity: Bound<'_, PyAny> =
                native_field(&declarations.get_item(position)?, "identity")?;
            inaccessible.push(if with_reasons {
                inaccessible_class.call1((
                    &owner,
                    identity,
                    classes.member(RecordId::InaccessibleReason, reason)?,
                ))?
            } else {
                identity
            });
        }
    }
    let result: RecordId = if with_reasons {
        RecordId::VisibilityResolution
    } else {
        RecordId::DeclarationVisibility
    };
    Ok(classes
        .model(result)?
        .call1((
            query,
            PyTuple::new(py, visible)?,
            PyTuple::new(py, inaccessible)?,
        ))?
        .unbind())
}

/// Visible and inaccessible declarations for an authored path, including declaration files.
#[pyfunction]
#[pyo3(name = "resolve_scope_path_visibility", signature = (*, lookup, path))]
pub(crate) fn scope_resolve_path_visibility(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    path: &Bound<'_, PyAny>,
) -> PyResult<(Py<PyTuple>, Py<PyTuple>)> {
    let text: String = path.str()?.extract()?;
    let normalized: String = match normalize_scope_path(&text) {
        Ok(normalized) => normalized,
        Err(failure) => {
            let rejected: RejectedPath = RejectedPath {
                failure,
                path: text,
            };
            return Err(scope_path_error(py, rejected_path_message(&rejected))?);
        }
    };
    let resource: ResourceRecord = ResourceRecord {
        identity: ResourceIdentity {
            kind: ResourceKind::Model,
            name: format!("<path:{normalized}>"),
        },
        path: normalized,
        ownership_root: OwnershipRoot {
            path: CURRENT_PATH.to_owned(),
            kind: OwnershipRootKind::Resource,
            resource_kind: Some(ResourceKind::Model),
        },
    };
    let (visible, inaccessible, _) = resolved(py, lookup, &resource)?;
    let declarations: Bound<'_, PyTuple> =
        tuple_attribute(&native_field(lookup, "index")?, "declarations")?;
    let pick = |positions: Vec<usize>| -> PyResult<Py<PyTuple>> {
        let records: Vec<Bound<'_, PyAny>> = positions
            .into_iter()
            .map(|position| declarations.get_item(position))
            .collect::<PyResult<_>>()?;
        Ok(PyTuple::new(py, records)?.unbind())
    };
    Ok((
        pick(
            visible
                .into_iter()
                .map(|(position, _, _)| position)
                .collect(),
        )?,
        pick(
            inaccessible
                .into_iter()
                .map(|(position, _)| position)
                .collect(),
        )?,
    ))
}
