//! Project Python files under the documented executable roots.

use std::path::Path;

use crate::python_paths::_helpers::python_files::python_files;
use crate::python_paths::_helpers::supported::is_supported;

/// Each supported Python file below `project_dir` as relative parts, sorted part by part.
pub fn supported_python_paths(project_dir: &Path) -> Vec<Vec<String>> {
    python_files(project_dir, &[])
        .into_iter()
        .filter(|parts| is_supported(parts))
        .collect()
}
