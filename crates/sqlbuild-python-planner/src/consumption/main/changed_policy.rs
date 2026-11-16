//! The policy of a `--changed` request, or None without it; triggered jobs resolve on `project`.

use crate::consumption::_helpers::policy;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use sqlbuild_state::consumption::models::ChangedPolicy;

/// The policy of a `--changed` request, or None without it; triggered jobs resolve on `project`.
#[inline]
pub fn changed_policy(
    project_dir: &Bound<'_, PyAny>,
    request: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<Option<ChangedPolicy>> {
    policy::changed_policy(project_dir, request, project)
}
