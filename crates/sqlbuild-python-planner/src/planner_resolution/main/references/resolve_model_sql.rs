//! One model's planned SQL: references resolved, cursor inputs filtered and intrinsics bound.

use crate::planner_resolution::_helpers::references::orchestration;
use crate::planner_resolution::models::ModelSqlRequest;
use crate::planner_resolution::models::Resolution;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// One model's planned SQL: references resolved, cursor inputs filtered and intrinsics bound.
#[inline]
pub fn resolve_model_sql<'py>(
    resolution: &Resolution<'py>,
    context: &Bound<'py, PyAny>,
    request: &ModelSqlRequest<'py>,
) -> PyResult<String> {
    orchestration::resolve_model_sql(resolution, context, request)
}
