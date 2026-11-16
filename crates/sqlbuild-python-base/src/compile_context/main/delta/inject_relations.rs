//! Give a delta's analysis session the whole project's relation facts and analysis order.

use crate::compile_context::_helpers::delta;
use sqlbuild_analysis::assembly::analysis_session::models::SessionRequest;

/// Give a delta's analysis session the whole project's relation facts and analysis order.
#[inline]
pub fn inject_relations(request: SessionRequest) -> pyo3::PyResult<SessionRequest> {
    delta::inject_relations(request)
}
