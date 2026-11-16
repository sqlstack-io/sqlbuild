//! `browse_scope_folders`: the direct child declaration-definition folders of `folder`.

use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::reports::scope_report_objects::browse_object;
use crate::scopes::_helpers::reports::scope_reports::LookupReports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use crate::scopes::_helpers::reports::scope_reports::folder_request;
use crate::scopes::_helpers::reports::scope_reports::lookup_reports;
use crate::scopes::models::ScopeClasses;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_scopes::scope_report::main::browse_scope_folders::browse_scope_folders;
use sqlbuild_scopes::scope_report::models::FolderRequest;

/// `browse_scope_folders`: the direct child declaration-definition folders of `folder`.
pub fn scope_browse(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: (String, Option<Bound<'_, PyAny>>, bool),
) -> PyResult<PyObject> {
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: FolderRequest = folder_request(&classes, request, None)?;
    let native: &NativeScopeLookup = reports.lookup.get();
    let result = browse_scope_folders(reports.source(native), &request);
    let diagnostics: Bound<'_, PyAny> =
        native_field(&native_field(lookup, "index")?, "diagnostics")?;
    Ok(browse_object(&classes, result, &diagnostics)?.unbind())
}
