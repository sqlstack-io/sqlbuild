//! Register the migration io functions and classes.

use crate::state_rows::_helpers::migrations::migration_io::read_renames;
use crate::state_rows::_helpers::migrations::migration_io::read_state_events;
use crate::state_rows::_helpers::migrations::migration_io::write_state_event;
use crate::state_rows::main::migrations::read_histories::read_histories;
use crate::state_rows::main::migrations::stored_columns_binding::stored_columns_binding;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the migration io functions and classes.
pub fn register_migration_io(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(read_state_events, module)?)?;
    module.add_function(wrap_pyfunction!(read_histories, module)?)?;
    module.add_function(wrap_pyfunction!(read_renames, module)?)?;
    module.add_function(wrap_pyfunction!(stored_columns_binding, module)?)?;
    module.add_function(wrap_pyfunction!(write_state_event, module)?)?;
    Ok(())
}
