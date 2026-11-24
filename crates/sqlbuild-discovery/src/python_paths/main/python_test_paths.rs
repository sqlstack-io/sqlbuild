//! Project Python tests, which `sqb test` runs with pytest and discovery never imports.

use std::path::Path;

use crate::python_paths::_helpers::python_files::python_files;
use crate::python_paths::constants::PYTHON_TEST_ROOTS;

/// Each Python file under `tests/orchestration/`, `tests/python/providers/` or `tests/python/nodes/` as relative parts, sorted part by part.
pub fn python_test_paths(project_dir: &Path) -> Vec<Vec<String>> {
    PYTHON_TEST_ROOTS
        .iter()
        .flat_map(|root| python_files(project_dir, root))
        .collect()
}
