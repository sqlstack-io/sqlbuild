//! `list_scope_declarations`: declarations recursively beneath one definition folder.

use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::reports::scope_report_objects::list_object;
use crate::scopes::_helpers::reports::scope_reports::LookupReports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use crate::scopes::_helpers::reports::scope_reports::failure_error;
use crate::scopes::_helpers::reports::scope_reports::folder_request;
use crate::scopes::_helpers::reports::scope_reports::lookup_reports;
use crate::scopes::models::ListArguments;
use crate::scopes::models::ScopeClasses;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_scopes::scope_report::main::list_scope_declarations::list_scope_declarations;
use sqlbuild_scopes::scope_report::models::FolderRequest;

/// `list_scope_declarations`: declarations recursively beneath one definition folder.
pub fn scope_list(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ListArguments<'_>,
) -> PyResult<PyObject> {
    let ListArguments {
        folder,
        filters,
        target,
        target_is_prospective,
    } = request;
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: FolderRequest =
        folder_request(&classes, (folder, target, target_is_prospective), filters)?;
    let native: &NativeScopeLookup = reports.lookup.get();
    match list_scope_declarations(reports.source(native), &request) {
        Ok(result) => {
            let diagnostics: Bound<'_, PyAny> =
                native_field(&native_field(lookup, "index")?, "diagnostics")?;
            Ok(list_object(&classes, result, &diagnostics)?.unbind())
        }
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}
