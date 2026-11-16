//! `path` relative to `project_dir` with `/` separators, or as given outside the project.

use std::path::Path;

/// `path` relative to `project_dir` with `/` separators, or as given outside the project.
pub fn project_relative_path(path: &Path, project_dir: &Path) -> String {
    let relative: &Path = path.strip_prefix(project_dir).unwrap_or(path);
    relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<String>>()
        .join("/")
}
