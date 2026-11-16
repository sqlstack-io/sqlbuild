//! Scoped declarations the resources at `path` use that resolve differently from `destination`.

use crate::scopes::_helpers::index::scope_lookup::NativeScopeLookup;
use crate::scopes::_helpers::reports::scope_reports::LookupReports;
use crate::scopes::_helpers::reports::scope_reports::failure_error;
use crate::scopes::_helpers::reports::scope_reports::lookup_reports;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_scopes::scope_report::main::rebound_scope_usages::rebound_scope_usages;

/// Scoped declarations the resources at `path` use that resolve differently from `destination`.
pub fn scope_rebound_usages(
    py: Python<'_>,
    lookup: &Bound<'_, PyAny>,
    paths: (&str, &str),
) -> PyResult<Option<Vec<String>>> {
    let reports: LookupReports = lookup_reports(lookup)?;
    let native: &NativeScopeLookup = reports.lookup.get();
    match rebound_scope_usages(reports.source(native), paths) {
        Ok(rebound) => Ok(rebound),
        Err(failure) => Err(failure_error(py, &failure)?),
    }
}
