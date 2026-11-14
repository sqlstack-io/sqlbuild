//! The project Python files the selected Python rules read, digested so the memo sees every edit.

use std::path::Path;

use sqlbuild_discovery::python_paths::main::supported_python_paths::supported_python_paths;

use crate::models::{EvaluateRequest, PythonSource};
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

fn read_source(project_dir: &Path, relative_path: String) -> Result<PythonSource, String> {
    let bytes: Vec<u8> = std::fs::read(project_dir.join(&relative_path))
        .map_err(|error| format!("{relative_path}: cannot read Python source: {error}"))?;
    let digest: String = blake3::hash(&bytes).to_hex().to_string();
    let text: String = String::from_utf8(bytes)
        .map_err(|_| format!("{relative_path}: Python source is not valid UTF-8"))?;
    Ok(PythonSource {
        relative_path,
        digest,
        text,
    })
}
