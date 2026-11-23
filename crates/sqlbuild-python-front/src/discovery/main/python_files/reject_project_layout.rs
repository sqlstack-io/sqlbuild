//! Raise `ProjectLayoutError` (D018) naming every entry outside the project layout.

use crate::discovery::_helpers::failures::raised::discovery_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_discovery::project_layout::main::layout_problems::layout_problems;
use sqlbuild_discovery::project_layout::models::LayoutProblem;
use std::path::PathBuf;

/// The extra-paths line every layout help ends with.
const EXTRA_PATHS_HELP: &str = "or list it in extra_paths in sqlbuild_project.toml if it belongs here on purpose";

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
    let message: String = problems
        .iter()
        .map(|problem| format!("'{}' is not part of a SQLBuild project", problem.path))
        .collect::<Vec<String>>()
        .join("\n");
    let mut folders: Vec<Option<&str>> = Vec::new();
    for problem in &problems {
        if !folders.contains(&problem.folder) {
            folders.push(problem.folder);
        }
    }
    let mut help: Vec<String> = folders
        .iter()
        .filter_map(|folder| {
            let problem: &LayoutProblem = problems.iter().find(|item| item.folder == *folder)?;
            let place: String = folder.map_or_else(String::new, |name| format!(" in {name}/"));
            Some(format!("move it under one of{place}: {}", problem.allowed.join(", ")))
        })
        .collect();
    help.push(EXTRA_PATHS_HELP.to_owned());
    Err(discovery_error(
        project_dir.py(),
        "project_layout",
        message,
        Some(help.join(",\n")),
    ))
}
