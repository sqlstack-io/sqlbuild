//! The calendars and sensors of the inventory's modules, imported by publication and by sensor evaluation.

use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;
use sqlbuild_discovery::tree::models::ProjectTree;
use sqlbuild_python_base::orchestration_reads::main::module_declarations::module_declarations;
use std::collections::BTreeSet;
use std::path::Path;

/// The calendars and sensors of the inventory's modules, imported by publication and by sensor evaluation.
pub fn python_declarations<'py>(
    py: Python<'py>,
    project_dir: &Path,
    inventory: &OrchestrationInventory,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let tree = ProjectTree::new(project_dir);
    let mut declarations: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    for module in inventory.calendars.iter().chain(&inventory.sensors) {
        for declaration in module_declarations(py, &tree.absolute(&module.path), project_dir)? {
            if seen.insert(declaration.as_ptr() as usize) {
                declarations.push(declaration);
            }
        }
    }
    Ok(declarations)
}
