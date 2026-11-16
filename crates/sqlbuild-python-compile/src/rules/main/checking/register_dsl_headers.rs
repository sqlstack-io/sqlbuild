//! Register the dsl headers functions and classes.

use crate::rules::_helpers::checking::dsl_headers::format_native_headers;
use crate::rules::_helpers::checking::dsl_headers::lint_native_headers;
use crate::rules::_helpers::checking::dsl_headers::measurement_body_ranges;
use crate::rules::_helpers::checking::dsl_headers::reject_unparseable_header_rewrites;
use crate::rules::_helpers::checking::dsl_headers::safe_format_files;
use crate::rules::_helpers::checking::dsl_headers::scan_headers;
use crate::rules::_helpers::checking::dsl_headers::sql_body_ranges;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the dsl headers functions and classes.
pub fn register_dsl_headers(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(scan_headers, module)?)?;
    module.add_function(wrap_pyfunction!(sql_body_ranges, module)?)?;
    module.add_function(wrap_pyfunction!(measurement_body_ranges, module)?)?;
    module.add_function(wrap_pyfunction!(lint_native_headers, module)?)?;
    module.add_function(wrap_pyfunction!(format_native_headers, module)?)?;
    module.add_function(wrap_pyfunction!(safe_format_files, module)?)?;
    module.add_function(wrap_pyfunction!(
        reject_unparseable_header_rewrites,
        module
    )?)?;
    Ok(())
}
