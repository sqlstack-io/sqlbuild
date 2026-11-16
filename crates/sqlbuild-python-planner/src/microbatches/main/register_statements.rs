//! Register the statements functions and classes.

use crate::microbatches::_helpers::statements::create_indexes;
use crate::microbatches::_helpers::statements::create_table;
use crate::microbatches::_helpers::statements::deterministic_event_id;
use crate::microbatches::_helpers::statements::existing_ids;
use crate::microbatches::_helpers::statements::insert_many;
use crate::microbatches::_helpers::statements::read_scope;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the statements functions and classes.
pub fn register_statements(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(insert_many, module)?)?;
    module.add_function(wrap_pyfunction!(existing_ids, module)?)?;
    module.add_function(wrap_pyfunction!(read_scope, module)?)?;
    module.add_function(wrap_pyfunction!(create_table, module)?)?;
    module.add_function(wrap_pyfunction!(create_indexes, module)?)?;
    module.add_function(wrap_pyfunction!(deterministic_event_id, module)?)
}
