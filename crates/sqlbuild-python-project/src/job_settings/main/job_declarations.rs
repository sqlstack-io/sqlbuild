//! Each job's `[settings]` and `[retry]` and each group's jobs declared for the command's target.

use crate::job_settings::_helpers::job_catalog;
use pyo3::prelude::{PyResult, Python};
use sqlbuild_analysis::job_settings::models::DeclaredRun;
use std::collections::BTreeMap;

/// Each job's `[settings]` and `[retry]` and each group's jobs declared for the command's target; none outside a command.
#[inline]
pub fn job_declarations(py: Python<'_>) -> PyResult<Option<BTreeMap<String, DeclaredRun>>> {
    job_catalog::job_declarations(py)
}
