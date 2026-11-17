//! Register the event writer functions and classes.

use crate::result_output::models::ExecutionEventWriter;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};

/// Register the event writer functions and classes.
pub fn register_event_writer(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<ExecutionEventWriter>()
}
