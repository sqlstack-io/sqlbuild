//! Register the scalars functions and classes.

use crate::cursor::_helpers::scalars::floor_value;
use crate::cursor::_helpers::scalars::max_bound;
use crate::cursor::_helpers::scalars::min_bound;
use crate::cursor::_helpers::scalars::next_value;
use crate::cursor::_helpers::scalars::observed_end;
use crate::cursor::_helpers::scalars::sentinel_from_token;
use crate::cursor::main::scalars::compare_values::compare_values;
use crate::cursor::main::scalars::parse::parse;
use crate::cursor::main::scalars::render_value::render_value;
use crate::cursor::main::scalars::sentinel_to_token::sentinel_to_token;
use crate::cursor::main::scalars::to_exclusive::to_exclusive;
use crate::cursor::main::scalars::to_inclusive::to_inclusive;
use crate::cursor::main::scalars::try_parse::try_parse;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the scalars functions and classes.
pub fn register_scalars(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(parse, module)?)?;
    module.add_function(wrap_pyfunction!(try_parse, module)?)?;
    module.add_function(wrap_pyfunction!(render_value, module)?)?;
    module.add_function(wrap_pyfunction!(sentinel_to_token, module)?)?;
    module.add_function(wrap_pyfunction!(sentinel_from_token, module)?)?;
    module.add_function(wrap_pyfunction!(compare_values, module)?)?;
    module.add_function(wrap_pyfunction!(max_bound, module)?)?;
    module.add_function(wrap_pyfunction!(min_bound, module)?)?;
    module.add_function(wrap_pyfunction!(floor_value, module)?)?;
    module.add_function(wrap_pyfunction!(next_value, module)?)?;
    module.add_function(wrap_pyfunction!(to_exclusive, module)?)?;
    module.add_function(wrap_pyfunction!(to_inclusive, module)?)?;
    module.add_function(wrap_pyfunction!(observed_end, module)?)?;
    Ok(())
}
