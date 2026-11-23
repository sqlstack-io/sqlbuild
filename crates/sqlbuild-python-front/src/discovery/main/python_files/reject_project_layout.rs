//! Raise `ProjectLayoutError` (D018) naming every entry outside the project layout.

use crate::discovery::_helpers::failures::raised::discovery_error;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_discovery::project_layout::main::layout_problems::layout_problems;
use sqlbuild_discovery::project_layout::models::LayoutProblem;
use std::path::PathBuf;

/// The extra-paths line every layout help ends with.
const EXTRA_PATHS_HELP: &str =
    "or list it in extra_paths in sqlbuild_project.toml if it belongs here on purpose";
/// The indent that lines a help continuation up under the first help line.
const HELP_CONTINUATION: &str = "\n          ";
/// The prefix of a further help line.
const HELP_LINE: &str = "\n  = help: ";

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
    for problem in &problems {
        if !folders.contains(&problem.folder) {
            folders.push(problem.folder);
        }
    }
    let lines: Vec<String> = folders
        .iter()
        .filter_map(|folder| {
            let problem: &LayoutProblem = problems.iter().find(|item| item.folder == *folder)?;
            Some(format!(
                "move it under one of: {},",
                problem.allowed.join(", ")
            ))
        })
        .collect();
    Err(discovery_error(
        project_dir.py(),
        "project_layout",
        message,
        Some(format!(
            "{}{HELP_CONTINUATION}{EXTRA_PATHS_HELP}",
            lines.join(HELP_LINE)
        )),
    ))
}
