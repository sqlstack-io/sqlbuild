//! Register the macro calls functions and classes.

use crate::render::_helpers::calls::macro_call_memo::MacroCallMemo;
use crate::render::_helpers::calls::macro_calls::find_macro_call_names;
use crate::render::_helpers::calls::macro_calls::scan_macro_call_sites;
use crate::render::_helpers::calls::macro_calls::splice_macro_calls;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use pyo3::wrap_pyfunction;

/// Register the macro calls functions and classes.
pub fn register_macro_calls(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(scan_macro_call_sites, module)?)?;
    module.add_function(wrap_pyfunction!(find_macro_call_names, module)?)?;
    module.add_function(wrap_pyfunction!(splice_macro_calls, module)?)?;
    module.add_class::<MacroCallMemo>()?;
    Ok(())
}
