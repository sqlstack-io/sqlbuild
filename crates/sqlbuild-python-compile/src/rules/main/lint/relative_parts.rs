//! The parts of `path` below `project_dir`, or `None` outside the project.

/// The parts of `path` below `project_dir`, or `None` outside the project.
pub fn relative_parts(
    path: &std::path::Path,
    project_dir: &std::path::Path,
) -> Option<Vec<String>> {
    match path.strip_prefix(project_dir) {
        Ok(relative) => Some(
            relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect(),
        ),
        Err(_) => None,
    }
}
