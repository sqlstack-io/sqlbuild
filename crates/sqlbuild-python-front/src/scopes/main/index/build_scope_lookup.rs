//! Python's `build_scope_lookup`: the canonical index, its mappings and the native lookup.

use crate::scopes::_helpers::index::scope_lookup::CanonicalPositions;
use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::index::scope_lookup::grouped;
use crate::scopes::_helpers::index::scope_lookup::inverse;
use crate::scopes::_helpers::index::scope_lookup::path_grouped;
use crate::scopes::_helpers::index::scope_lookup::picked;
use crate::scopes::_helpers::index::scope_lookup::scope_path_error;
use crate::scopes::_helpers::index::scope_lookup::visibility_index;
use crate::scopes::_helpers::index::scope_objects::{read_index, tuple_attribute};
use crate::scopes::models::ScopeClasses;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyTuple;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_scopes::scope_index::main::rejected_path_message::rejected_path_message;
use sqlbuild_scopes::scope_index::main::report_lookup::report_lookup;
use sqlbuild_scopes::scope_index::models::{ReportLookup, ReportRecords};
use sqlbuild_scopes::scope_report::main::cached_report_records::cached_report_records;

/// Python's `build_scope_lookup`: the canonical index, its mappings and the native lookup.
#[pyfunction]
#[pyo3(signature = (*, index))]
pub fn build_scope_lookup(py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let records: ReportRecords = cached_report_records(&read_index(index)?);
    let built = py
        .compiler_detach(|| Ok(report_lookup(&records)))
        .map_err(compiler_error)?;
    let lookup: ReportLookup = match built {
        Ok(lookup) => lookup,
        Err(rejected) => return Err(scope_path_error(py, rejected_path_message(&rejected))?),
    };
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let resources: Bound<'_, PyTuple> = tuple_attribute(index, "resources")?;
    let declarations: Bound<'_, PyTuple> = tuple_attribute(index, "declarations")?;
    let usages: Bound<'_, PyTuple> = tuple_attribute(index, "usages")?;
    let grants: Bound<'_, PyTuple> = tuple_attribute(index, "grants")?;
    let canonical_declarations: Bound<'_, PyTuple> = picked(&declarations, &lookup.declarations)?;
    let canonical_index: Bound<'_, PyAny> = classes.model(RecordId::ScopeIndex)?.call1((
        picked(&tuple_attribute(index, "ownership_roots")?, &lookup.roots)?,
        picked(&resources, &lookup.resources)?,
        &canonical_declarations,
        picked(&usages, &lookup.usages)?,
        picked(&grants, &lookup.grants)?,
        native_field(index, "visibility")?,
        native_field(index, "inaccessible")?,
        picked(&tuple_attribute(index, "diagnostics")?, &lookup.diagnostics)?,
        native_field(index, "completeness")?,
    ))?;
    let groups = &lookup.groups;
    let mappings: [Bound<'_, PyAny>; 6] = [
        grouped(&groups.resources, &resources, "identity")?,
        path_grouped(
            &groups.resources_by_path,
            &resources,
            &lookup.resource_paths,
        )?,
        grouped(&groups.declarations, &declarations, "identity")?,
        grouped(&groups.usages_by_consumer, &usages, "consumer")?,
        grouped(&groups.usages_by_declaration, &usages, "declaration")?,
        grouped(&groups.grants_by_resource, &grants, "resource")?,
    ];
    let visibility: Bound<'_, PyAny> =
        visibility_index(&classes, &canonical_declarations, &lookup.visibility)?;
    let canonical: CanonicalPositions = CanonicalPositions {
        resources: inverse(&lookup.resources),
        declarations: inverse(&lookup.declarations),
        grants: inverse(&lookup.grants),
    };
    let native: Py<NativeScopeLookup> = Py::new(
        py,
        NativeScopeLookup {
            records,
            lookup,
            canonical,
        },
    )?;
    let [
        by_identity,
        by_path,
        by_declaration,
        by_consumer,
        by_used,
        by_resource,
    ] = mappings;
    Ok(classes
        .model(RecordId::ScopeLookup)?
        .call1((
            canonical_index,
            by_identity,
            by_path,
            by_declaration,
            by_consumer,
            by_used,
            by_resource,
            visibility,
            native,
        ))?
        .unbind())
}
