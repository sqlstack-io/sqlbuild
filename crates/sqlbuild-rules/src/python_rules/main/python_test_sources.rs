//! The project Python test files the selected Python test rules read.

use std::path::Path;

use sqlbuild_discovery::python_paths::main::python_test_paths::python_test_paths;

use crate::models::{EvaluateRequest, PythonSource};
use crate::python_rules::_helpers::engine::source_files::read_source;
use crate::python_rules::constants::PYTHON_RULES;
use crate::python_rules::models::PythonFiles;
use crate::rules::main::request_selects::request_selects;

/// Every project Python test file when the request selects a Python test rule, otherwise none.
pub(crate) fn python_test_sources(request: &EvaluateRequest) -> Result<Vec<PythonSource>, String> {
    let test_rule = |code: &str| {
        PYTHON_RULES
            .iter()
            .any(|rule| rule.code == code && rule.applies == PythonFiles::Tests)
    };
    if !request_selects(request, |rule| test_rule(&rule.code))? {
        return Ok(Vec::new());
    }
    let project_dir: &Path = Path::new(&request.project_dir);
    python_test_paths(project_dir)
        .into_iter()
        .map(|parts| read_source(project_dir, parts.join("/")))
        .collect()
}
