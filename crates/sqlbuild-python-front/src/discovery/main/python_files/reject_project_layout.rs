//! Raise `ProjectLayoutError` (D018) naming every entry outside the project layout.

use crate::discovery::_helpers::failures::raised::discovery_error;
use crate::discovery::constants::{
    LAYOUT_EXTRA_PATHS_HELP, LAYOUT_HELP_CONTINUATION, LAYOUT_HELP_LINE,
};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_discovery::project_layout::main::layout_problems::layout_problems;
use sqlbuild_discovery::project_layout::models::LayoutProblem;
use std::path::PathBuf;

/// Raise `ProjectLayoutError` for each entry of `project_dir` outside the expected layout.
pub fn reject_project_layout(
    project_dir: &Bound<'_, PyAny>,
    extra_paths: &[String],
) -> PyResult<()> {
    let root: PathBuf = project_dir.extract()?;
    let problems: Vec<LayoutProblem> = layout_problems(&root, extra_paths);
    if problems.is_empty() {
        return Ok(());
    }
    let quoted: Vec<String> = problems
        .iter()
        .map(|problem| format!("'{}'", problem.path))
        .collect();
    let message: String = match quoted.as_slice() {
        [only] => format!("{only} is not part of a SQLBuild project"),
        [rest @ .., final_entry] => format!(
            "{} and {final_entry} are not part of a SQLBuild project",
            rest.join(", ")
        ),
        [] => String::new(),
    };
    let mut folders: Vec<Option<&str>> = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for problem in &problems {
        if !folders.contains(&problem.folder) {
            folders.push(problem.folder);
            lines.push(format!(
                "move it under one of: {},",
                problem.allowed.join(", ")
            ));
        }
    }
    Err(discovery_error(
        project_dir.py(),
        "project_layout",
        message,
        Some(format!(
            "{}{LAYOUT_HELP_CONTINUATION}{LAYOUT_EXTRA_PATHS_HELP}",
            lines.join(LAYOUT_HELP_LINE)
        )),
    ))
}
