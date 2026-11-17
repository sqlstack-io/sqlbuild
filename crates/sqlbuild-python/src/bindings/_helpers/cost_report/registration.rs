//! Register the cost run store and the cost reports.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::cost_report::degradation;
use crate::bindings::_helpers::cost_report::exception_types;
use crate::bindings::_helpers::cost_report::functions;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    degradation::register(module)?;
    exception_types::register(module)?;
    functions::register(module)
}
