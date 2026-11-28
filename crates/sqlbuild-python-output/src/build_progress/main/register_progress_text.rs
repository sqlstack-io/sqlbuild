//! Register the progress text functions and classes.

use crate::build_progress::_helpers::progress_text::VerboseInspectionWriter;
use crate::build_progress::_helpers::progress_text::{
    format_expectation_detail, format_expectation_name,
};
use crate::build_progress::main::format_execution_header::format_execution_header;
use crate::build_progress::main::write_execution_header::write_execution_header;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the progress text functions and classes.
pub fn register_progress_text(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<VerboseInspectionWriter>()?;
    module.add_function(wrap_pyfunction!(format_execution_header, module)?)?;
    module.add_function(wrap_pyfunction!(write_execution_header, module)?)?;
    module.add_function(wrap_pyfunction!(format_expectation_name, module)?)?;
    module.add_function(wrap_pyfunction!(format_expectation_detail, module)?)
}
