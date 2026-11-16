//! Register the events functions and classes.

use crate::state_rows::_helpers::event_rows::events::column_id;
use crate::state_rows::_helpers::event_rows::events::migration_id;
use crate::state_rows::_helpers::event_rows::events::old_name_id;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the events functions and classes.
pub fn register_events(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(migration_id, module)?)?;
    module.add_function(wrap_pyfunction!(column_id, module)?)?;
    module.add_function(wrap_pyfunction!(old_name_id, module)?)?;
    Ok(())
}
