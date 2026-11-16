//! Register the project tree queries functions and classes.

use crate::rules::_helpers::checking::project_tree_queries::project_tree_child_nodes;
use crate::rules::_helpers::checking::project_tree_queries::project_tree_descendant_nodes;
use crate::rules::_helpers::checking::project_tree_queries::project_tree_glob_nodes;
use crate::rules::_helpers::checking::project_tree_queries::project_tree_read_text;
use crate::rules::_helpers::checking::project_tree_queries::project_tree_relative_parts;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the project tree queries functions and classes.
pub fn register_project_tree_queries(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(project_tree_child_nodes, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_descendant_nodes, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_glob_nodes, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_relative_parts, module)?)?;
    module.add_function(wrap_pyfunction!(project_tree_read_text, module)?)?;
    Ok(())
}
