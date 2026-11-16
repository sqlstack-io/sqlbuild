//! Register the relations functions and classes.

use crate::references::_helpers::relations::extract_relation_names;
use crate::references::_helpers::relations::runtime_project_relations;
use crate::references::main::match_project_relation::match_project_relation;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the relations functions and classes.
pub fn register_relations(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(extract_relation_names, module)?)?;
    module.add_function(wrap_pyfunction!(match_project_relation, module)?)?;
    module.add_function(wrap_pyfunction!(runtime_project_relations, module)?)?;
    Ok(())
}
