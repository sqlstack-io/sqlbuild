//! `report_scope_index_errors`: each error-severity finding of `index`, keyed for the collector.

use crate::scopes::_helpers::reports::scope_errors;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// `report_scope_index_errors`: each error-severity finding of `index`, keyed for the collector.
#[inline]
pub fn report_scope_index_errors(index: &Bound<'_, PyAny>) -> PyResult<()> {
    scope_errors::report_scope_index_errors(index)
}
