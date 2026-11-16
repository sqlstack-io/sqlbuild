//! Each calendar request's trigger, with each (calendar, timezone) pair evaluated once.

use crate::native_host::_helpers::orchestration_triggers;
use crate::native_host::models::CalendarRequest;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use sqlbuild_orchestration::declarations::models::Definitions;
use std::collections::BTreeMap;
use std::path::Path;

/// Each calendar request's trigger, with each (calendar, timezone) pair evaluated once.
#[inline]
pub fn compile_calendars(
    py: Python<'_>,
    project_dir: &Path,
    (calendars, requests): (&BTreeMap<String, Bound<'_, PyAny>>, Vec<CalendarRequest>),
    definitions: Definitions,
) -> PyResult<Definitions> {
    orchestration_triggers::compile_calendars(py, project_dir, (calendars, requests), definitions)
}
