//! The project Python files the selected Python rules read, digested so the memo sees every edit.

use std::path::Path;

use sqlbuild_discovery::python_paths::main::supported_python_paths::supported_python_paths;

use crate::models::{EvaluateRequest, PythonSource};
use crate::python_rules::_helpers::engine::source_files::read_source;
use crate::rules::main::request_selects::request_selects;

/// Every supported project Python file when the request selects a Python rule, otherwise none.
pub(crate) fn python_sources(request: &EvaluateRequest) -> Result<Vec<PythonSource>, String> {
    if !request_selects(request, |rule| rule.scope.evaluates_python())? {
        return Ok(Vec::new());
    }
    let project_dir: &Path = Path::new(&request.project_dir);
    supported_python_paths(project_dir)
        .into_iter()
        .map(|parts| read_source(project_dir, parts.join("/")))
        .collect()
}
