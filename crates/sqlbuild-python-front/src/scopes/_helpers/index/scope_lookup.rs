//! The scope lookup of a full scope index: canonical orders, mappings, targets and visibility.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use pyo3::{pyclass, pyfunction};
use sqlbuild_scopes::scope_index::main::identity_failure_message::identity_failure_message;
use sqlbuild_scopes::scope_index::main::normalize_scope_path::normalize_scope_path;
use sqlbuild_scopes::scope_index::main::rejected_path_message::rejected_path_message;
use sqlbuild_scopes::scope_index::main::report_target::report_target;
use sqlbuild_scopes::scope_index::main::resolve_report_visibility::resolve_report_visibility;
use sqlbuild_scopes::scope_index::models::{
    GrantKind, KeyedGroups, RejectedPath, ReportLookup, ReportRecords, ReportTarget,
    ResolvedVisibility, ResolvedVisible, ResourceRecord, TargetFailure, TargetMatch,
    VisibilityGroups,
};

use crate::scopes::_helpers::declarations::placement::{QualifiedRow, qualified_identity};
use crate::scopes::_helpers::index::scope_objects::resource_identity_object;
use crate::scopes::models::ScopeClasses;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;

const TYPES_MODULE: &str = "types";

/// `(declarations, canonical positions)` matched, else identity and path error messages.
pub(crate) struct TargetRows {
    pub(crate) declarations: bool,
    pub(crate) positions: Vec<usize>,
    pub(crate) identity_error: Option<String>,
    pub(crate) path_error: Option<String>,
}
/// Visible `(position, reason, grant)`, inaccessible `(position, reason)`, else the path error.
pub(crate) type ResolvedRows = (
    Vec<(usize, &'static str, Option<usize>)>,
    Vec<(usize, &'static str)>,
    Option<String>,
);

/// One full scope index's records and lookup, kept for targets and classification.
#[pyclass(module = "sqlbuild._native", frozen)]
pub(crate) struct NativeScopeLookup {
    pub(crate) records: ReportRecords,
    pub(crate) lookup: ReportLookup,
    /// Each input resource, declaration and grant's canonical position.
    pub(crate) canonical: CanonicalPositions,
}

/// The inverse of the lookup's canonical orders.
pub(crate) struct CanonicalPositions {
    pub(crate) resources: Vec<usize>,
    pub(crate) declarations: Vec<usize>,
    pub(crate) grants: Vec<usize>,
}

pub(crate) fn inverse(order: &[usize]) -> Vec<usize> {
    let mut positions: Vec<usize> = vec![0; order.len()];
    for (canonical, input) in order.iter().enumerate() {
        positions[*input] = canonical;
    }
    positions
}

pub(crate) fn grant_kind(value: &str) -> PyResult<GrantKind> {
    match value {
        "expected_model" => Ok(GrantKind::ExpectedModel),
        "tested_macro" => Ok(GrantKind::TestedMacro),
        _ => Err(pyo3::exceptions::PyValueError::new_err(
            "invalid scope grant kind",
        )),
    }
}

/// Input positions as canonical positions.
pub(crate) fn canonical(order: &[usize], positions: Vec<usize>) -> Vec<usize> {
    positions
        .into_iter()
        .map(|position| order[position])
        .collect()
}

impl NativeScopeLookup {
    /// The records `target` names: an identity row, else qualified identity or path text.
    pub(crate) fn target(
        &self,
        identity: Option<QualifiedRow>,
        text: &str,
    ) -> PyResult<TargetRows> {
        let target: ReportTarget = match identity {
            Some(row) => ReportTarget::Identity(qualified_identity(row)?),
            None => ReportTarget::Text(text.to_owned()),
        };
        Ok(match report_target(&self.records, &self.lookup, &target) {
            Ok(TargetMatch::Resources(positions)) => TargetRows {
                declarations: false,
                positions: canonical(&self.canonical.resources, positions),
                identity_error: None,
                path_error: None,
            },
            Ok(TargetMatch::Declarations(positions)) => TargetRows {
                declarations: true,
                positions: canonical(&self.canonical.declarations, positions),
                identity_error: None,
                path_error: None,
            },
            Err(TargetFailure::Identity(failure)) => TargetRows {
                declarations: false,
                positions: Vec::new(),
                identity_error: Some(identity_failure_message(&failure, text)),
                path_error: None,
            },
            Err(TargetFailure::Path(rejected)) => TargetRows {
                declarations: false,
                positions: Vec::new(),
                identity_error: None,
                path_error: Some(rejected_path_message(&rejected)),
            },
        })
    }

    /// Python's `_classify_resource` of one resource, with inaccessible reasons.
    pub(crate) fn resolve(
        &self,
        py: Python<'_>,
        resource: &ResourceRecord,
    ) -> PyResult<ResolvedRows> {
        let (records, lookup) = (&self.records, &self.lookup);
        let resolved = py
            .compiler_detach(|| Ok(resolve_report_visibility(records, lookup, resource)))
            .map_err(compiler_error)?;
        Ok(match resolved {
            Ok(resolved) => resolved_rows(resolved, &self.canonical.grants),
            Err(rejected) => (
                Vec::new(),
                Vec::new(),
                Some(rejected_path_message(&rejected)),
            ),
        })
    }
}

fn visible_row(entry: ResolvedVisible, grants: &[usize]) -> (usize, &'static str, Option<usize>) {
    let grant: Option<usize> = entry.grant.map(|grant| grants[grant]);
    (entry.position, entry.reason.as_str(), grant)
}

fn resolved_rows(resolved: ResolvedVisibility, grants: &[usize]) -> ResolvedRows {
    let visible: Vec<(usize, &'static str, Option<usize>)> = resolved
        .visible
        .into_iter()
        .map(|entry| visible_row(entry, grants))
        .collect();
    (
        visible,
        resolved
            .inaccessible
            .into_iter()
            .map(|(position, reason)| (position, reason.as_str()))
            .collect(),
        None,
    )
}

pub(crate) fn picked<'py>(
    values: &Bound<'py, PyTuple>,
    order: &[usize],
) -> PyResult<Bound<'py, PyTuple>> {
    let items: Vec<Bound<'py, PyAny>> = order
        .iter()
        .map(|position| values.get_item(*position))
        .collect::<PyResult<_>>()?;
    PyTuple::new(values.py(), items)
}

fn mapping_proxy<'py>(entries: &Bound<'py, PyDict>) -> PyResult<Bound<'py, PyAny>> {
    entries
        .py()
        .import(TYPES_MODULE)?
        .getattr("MappingProxyType")?
        .call1((entries,))
}

/// Python's `_mapping`: each group keyed by its first record's `key` attribute.
pub(crate) fn grouped<'py>(
    groups: &KeyedGroups,
    records: &Bound<'py, PyTuple>,
    key: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let entries: Bound<'py, PyDict> = PyDict::new(records.py());
    for group in &groups.groups {
        let members: Bound<'py, PyTuple> = picked(records, group)?;
        entries.set_item(members.get_item(0)?.getattr(key)?, members)?;
    }
    mapping_proxy(&entries)
}

pub(crate) fn path_grouped<'py>(
    groups: &KeyedGroups,
    records: &Bound<'py, PyTuple>,
    paths: &[String],
) -> PyResult<Bound<'py, PyAny>> {
    let entries: Bound<'py, PyDict> = PyDict::new(records.py());
    for group in &groups.groups {
        entries.set_item(paths[group[0]].as_str(), picked(records, group)?)?;
    }
    mapping_proxy(&entries)
}

fn positions_by<'py>(
    py: Python<'py>,
    entries: &[(String, Vec<usize>)],
) -> PyResult<Bound<'py, PyAny>> {
    let mapping: Bound<'py, PyDict> = PyDict::new(py);
    for (key, positions) in entries {
        mapping.set_item(key.as_str(), PyTuple::new(py, positions)?)?;
    }
    mapping_proxy(&mapping)
}

pub(crate) fn visibility_index<'py>(
    classes: &ScopeClasses<'py>,
    canonical_declarations: &Bound<'py, PyTuple>,
    visibility: &VisibilityGroups,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = canonical_declarations.py();
    let identities: Vec<Bound<'py, PyAny>> = canonical_declarations
        .iter()
        .map(|record| native_field(&record, "identity"))
        .collect::<PyResult<_>>()?;
    let private: Bound<'py, PyDict> = PyDict::new(py);
    for (owner, positions) in &visibility.private {
        private.set_item(
            resource_identity_object(classes, owner)?,
            PyTuple::new(py, positions)?,
        )?;
    }
    classes.model(RecordId::DeclarationVisibilityIndex)?.call1((
        PyTuple::new(py, identities)?,
        PyTuple::new(py, &visibility.global)?,
        mapping_proxy(&private)?,
        positions_by(py, &visibility.local)?,
        positions_by(py, &visibility.inherited)?,
    ))
}

pub(crate) fn scope_path_error(py: Python<'_>, message: String) -> PyResult<PyErr> {
    Ok(PyErr::from_value(
        error_class(py, ErrorId::InvalidScopePath)?.call1((message,))?,
    ))
}

/// The canonical project-relative POSIX path; absolute or escaping paths raise.
#[pyfunction]
#[pyo3(name = "normalize_scope_path", signature = (*, path))]
pub(crate) fn scope_normalize_path(py: Python<'_>, path: &Bound<'_, PyAny>) -> PyResult<String> {
    let path: String = path.str()?.extract()?;
    match normalize_scope_path(&path) {
        Ok(normalized) => Ok(normalized),
        Err(failure) => Err(scope_path_error(
            py,
            rejected_path_message(&RejectedPath { failure, path }),
        )?),
    }
}
