//! A declaration's file relative to the project; code outside the project keeps its location private.

use crate::orchestration_reads::_helpers::orchestration_declarations;
use pyo3::prelude::PyResult;
use sqlbuild_orchestration::declarations::models::DeclaredAt;
use std::path::Path;

/// A declaration's file relative to the project; code outside the project keeps its location private.
#[inline]
pub fn declared_at(
    project_dir: &Path,
    written: Option<(String, u32)>,
) -> PyResult<Option<DeclaredAt>> {
    orchestration_declarations::declared_at(project_dir, written)
}
