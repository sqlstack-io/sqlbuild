//! Register the ddl effects functions and classes.

use crate::sqltext::_helpers::ddl_effects::qualified_relation_name_key;
use crate::sqltext::_helpers::ddl_effects::relation_name_key;
use crate::sqltext::_helpers::ddl_effects::statement_metadata_effect;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the ddl effects functions and classes.
pub fn register_ddl_effects(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(statement_metadata_effect, module)?)?;
    module.add_function(wrap_pyfunction!(relation_name_key, module)?)?;
    module.add_function(wrap_pyfunction!(qualified_relation_name_key, module)?)?;
    Ok(())
}
