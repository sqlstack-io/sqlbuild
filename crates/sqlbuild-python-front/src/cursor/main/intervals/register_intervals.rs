//! Register the intervals functions and classes.

use crate::cursor::_helpers::intervals::cap_end;
use crate::cursor::_helpers::intervals::cap_start;
use crate::cursor::_helpers::intervals::clamp_intervals;
use crate::cursor::_helpers::intervals::merge_intervals;
use crate::cursor::_helpers::intervals::merge_raw_intervals;
use crate::cursor::_helpers::intervals::split;
use crate::cursor::main::intervals::interval_validation::interval_validation;
use crate::cursor::main::intervals::partition::partition;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the intervals functions and classes.
pub fn register_intervals(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(interval_validation, module)?)?;
    module.add_function(wrap_pyfunction!(partition, module)?)?;
    module.add_function(wrap_pyfunction!(clamp_intervals, module)?)?;
    module.add_function(wrap_pyfunction!(merge_intervals, module)?)?;
    module.add_function(wrap_pyfunction!(merge_raw_intervals, module)?)?;
    module.add_function(wrap_pyfunction!(split, module)?)?;
    module.add_function(wrap_pyfunction!(cap_start, module)?)?;
    module.add_function(wrap_pyfunction!(cap_end, module)?)?;
    Ok(())
}
