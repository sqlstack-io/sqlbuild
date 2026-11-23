//! Python node files named after what they declare, one declaration file per unit folder.

use crate::node_files::_helpers::problems::{module_problem, unit_folder_problem};
use crate::node_files::models::NodeModule;

/// The first problem with where `modules` declare their nodes, as a message, if any.
#[must_use]
pub fn node_file_problems(modules: &[NodeModule]) -> Option<String> {
    for module in modules {
        if let Some(problem) = module_problem(module) {
            return Some(problem);
        }
    }
    unit_folder_problem(modules)
}
