//! A `ScopeReport`, `ScopeBrowseResult` or, otherwise, `ScopeListResult` as its native result.

use crate::scopes::_helpers::reports::scope_report_objects;
use crate::scopes::models::ScopeClasses;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_scopes::scope_report::models::ScopeResult;

/// A `ScopeReport`, `ScopeBrowseResult` or, otherwise, `ScopeListResult` as its native result.
#[inline]
pub fn read_result(classes: &ScopeClasses<'_>, result: &Bound<'_, PyAny>) -> PyResult<ScopeResult> {
    scope_report_objects::read_result(classes, result)
}
