//! `query_scope_report`: one existing target or prospective path; invalid paths raise.

use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::reports::scope_report_objects::{
    read_filters, read_optional_target, report_object,
};
use crate::scopes::_helpers::reports::scope_reports::LookupReports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use crate::scopes::_helpers::reports::scope_reports::failure_error;
use crate::scopes::_helpers::reports::scope_reports::lookup_reports;
use crate::scopes::_helpers::reports::scope_reports::path_text;
use crate::scopes::models::ReportArguments;
use crate::scopes::models::ScopeClasses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_scopes::scope_report::main::build_scope_report::build_scope_report;
use sqlbuild_scopes::scope_report::models::ReportRequest;

/// `query_scope_report`: one existing target or prospective path; invalid paths raise.
pub fn scope_report(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ReportArguments<'_>,
) -> PyResult<PyObject> {
    let ReportArguments {
        target,
        at,
        directory,
        filters,
    } = request;
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let selected: Bound<'_, PyAny> = match filters.filter(|filters| !filters.is_none()) {
        Some(filters) => filters,
        None => classes.model(RecordId::ScopeReportFilters)?.call0()?,
    };
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: ReportRequest = ReportRequest {
        target: read_optional_target(&classes, target.as_ref())?,
        at: path_text(at)?,
        directory,
        filters: read_filters(&selected)?,
    };
    let native: &NativeScopeLookup = reports.lookup.get();
    match build_scope_report(reports.source(native), &request) {
        Ok(report) => {
            let diagnostics: Bound<'_, PyAny> =
                native_field(&native_field(lookup, "index")?, "diagnostics")?;
            Ok(report_object(&classes, report, (&selected, &diagnostics))?.unbind())
        }
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}
