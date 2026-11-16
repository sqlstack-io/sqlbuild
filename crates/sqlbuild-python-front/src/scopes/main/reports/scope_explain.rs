//! `explain_scope_declaration`: one qualified declaration in an optional context.

use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::reports::scope_report_objects::{
    diagnostics_object, explanation_object, read_declaration_target, read_optional_target,
};
use crate::scopes::_helpers::reports::scope_reports::LookupReports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use crate::scopes::_helpers::reports::scope_reports::failure_error;
use crate::scopes::_helpers::reports::scope_reports::lookup_reports;
use crate::scopes::_helpers::reports::scope_reports::path_text;
use crate::scopes::_helpers::reports::scope_reports::raised;
use crate::scopes::models::ExplainArguments;
use crate::scopes::models::ScopeClasses;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_scopes::scope_report::main::explain_scope_declaration::explain_scope_declaration;
use sqlbuild_scopes::scope_report::models::ExplainRequest;

/// `explain_scope_declaration`: one qualified declaration in an optional context.
pub fn scope_explain(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    request: ExplainArguments<'_>,
) -> PyResult<(PyObject, PyObject)> {
    let ExplainArguments {
        declaration,
        target,
        at,
        directory,
    } = request;
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: ExplainRequest = ExplainRequest {
        declaration: read_declaration_target(&classes, &declaration)?,
        target: read_optional_target(&classes, target.as_ref())?,
        at: path_text(at)?,
        directory,
    };
    let native: &NativeScopeLookup = reports.lookup.get();
    match explain_scope_declaration(reports.source(native), &request) {
        Ok((explanation, diagnostics)) => {
            let index_diagnostics: Bound<'_, PyAny> =
                native_field(&native_field(lookup, "index")?, "diagnostics")?;
            Ok((
                explanation_object(&classes, explanation)?.unbind(),
                diagnostics_object(&classes, &index_diagnostics, raised(diagnostics))?.unbind(),
            ))
        }
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}
