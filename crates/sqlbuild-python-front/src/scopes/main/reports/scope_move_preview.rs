//! `preview_scope_move`: visibility deltas of a resource move, without mutating the lookup.

use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::reports::scope_report_objects::{
    diagnostics_object, move_object, read_target,
};
use crate::scopes::_helpers::reports::scope_reports::LookupReports;
use crate::scopes::_helpers::reports::scope_reports::PyObject;
use crate::scopes::_helpers::reports::scope_reports::failure_error;
use crate::scopes::_helpers::reports::scope_reports::lookup_reports;
use crate::scopes::_helpers::reports::scope_reports::raised;
use crate::scopes::models::ScopeClasses;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_scopes::scope_report::main::preview_scope_move::preview_scope_move;
use sqlbuild_scopes::scope_report::models::MoveRequest;

/// `preview_scope_move`: visibility deltas of a resource move, without mutating the lookup.
#[pyfunction]
#[pyo3(name = "preview_scope_move", signature = (*, lookup, resource, destination))]
pub fn scope_move_preview(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    resource: &Bound<'_, PyAny>,
    destination: &Bound<'_, PyAny>,
) -> PyResult<(Option<PyObject>, PyObject)> {
    let classes: ScopeClasses<'_> = ScopeClasses::new(py)?;
    let reports: LookupReports = lookup_reports(lookup)?;
    let request: MoveRequest = MoveRequest {
        resource: read_target(&classes, resource)?,
        destination: destination.str()?.extract()?,
    };
    let native: &NativeScopeLookup = reports.lookup.get();
    match preview_scope_move(reports.source(native), &request) {
        Ok((preview, diagnostics)) => {
            let index_diagnostics: Bound<'_, PyAny> =
                native_field(&native_field(lookup, "index")?, "diagnostics")?;
            Ok((
                preview
                    .map(|preview| Ok::<_, PyErr>(move_object(&classes, preview)?.unbind()))
                    .transpose()?,
                diagnostics_object(&classes, &index_diagnostics, raised(diagnostics))?.unbind(),
            ))
        }
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}
