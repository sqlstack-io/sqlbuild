//! Register the project lineage functions and classes.

use crate::lineage::main::build_direct_semantic_uses::build_direct_semantic_uses;
use crate::lineage::main::build_project_column_lineage::build_project_column_lineage;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the project lineage functions and classes.
pub fn register_project_lineage(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_project_column_lineage, module)?)?;
    module.add_function(wrap_pyfunction!(build_direct_semantic_uses, module)?)?;
    Ok(())
}
