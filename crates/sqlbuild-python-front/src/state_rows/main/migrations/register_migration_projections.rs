//! Register the migration projections functions and classes.

use crate::state_rows::_helpers::migrations::migration_projections::newest_column_binding;
use crate::state_rows::_helpers::migrations::migration_projections::newest_move_binding;
use crate::state_rows::_helpers::migrations::migration_projections::old_name_views_binding;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the migration projections functions and classes.
pub fn register_migration_projections(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(newest_move_binding, module)?)?;
    module.add_function(wrap_pyfunction!(newest_column_binding, module)?)?;
    module.add_function(wrap_pyfunction!(old_name_views_binding, module)?)?;
    Ok(())
}
