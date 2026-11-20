//! The env file this process tree loaded, as messages show it.

use std::path::Path;

use sqlbuild_core::process_environment::main::env_file_marker::env_file_marker;

/// The env file's path relative to the project, such as `.env`; `None` when the project loads none.
pub fn env_file_display(project_dir: &Path) -> Option<String> {
    let path = env_file_marker()?.path?;
    let root = project_dir
        .canonicalize()
        .unwrap_or_else(|_| project_dir.to_path_buf());
    Some(
        Path::new(&path)
            .strip_prefix(&root)
            .map_or(path.clone(), |relative| {
                relative.to_string_lossy().into_owned()
            }),
    )
}
