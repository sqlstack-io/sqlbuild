//! Register the display functions and classes.

use crate::plan_output::_helpers::display::format_waiting_on_empty_inputs;
use crate::plan_output::main::aligned_timestamp_bounds::aligned_timestamp_bounds;
use crate::plan_output::main::cursor_bound_display::cursor_bound_display;
use crate::plan_output::main::inclusive_cursor_end::inclusive_cursor_end;
use crate::plan_output::main::resolve_effective_microbatch_batch_size::resolve_effective_microbatch_batch_size;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the display functions and classes.
pub fn register_display(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(inclusive_cursor_end, module)?)?;
    module.add_function(wrap_pyfunction!(cursor_bound_display, module)?)?;
    module.add_function(wrap_pyfunction!(format_waiting_on_empty_inputs, module)?)?;
    module.add_function(wrap_pyfunction!(
        resolve_effective_microbatch_batch_size,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(aligned_timestamp_bounds, module)?)?;
    Ok(())
}
