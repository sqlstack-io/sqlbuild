//! Register the terminal index functions and classes.

use crate::result_output::_helpers::events::terminal_index::CONTEXT_ATTRIBUTE;
use crate::result_output::_helpers::events::terminal_index::CONTEXT_NAME;
use crate::result_output::_helpers::events::terminal_index::TerminalEventClaim;
use crate::result_output::main::events::current_terminal_event_index::current_terminal_event_index;
use crate::result_output::main::events::terminal_event_index_scope::terminal_event_index_scope;
use crate::result_output::models::TerminalEventIndex;
use crate::result_output::models::TerminalEventIndexScope;
use pyo3::prelude::{Bound, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::wrap_pyfunction;

/// Register the terminal index functions and classes.
pub fn register_terminal_index(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let py: Python<'_> = module.py();
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    pyo3::types::PyDictMethods::set_item(&keywords, "default", py.None())?;
    module.add(
        CONTEXT_ATTRIBUTE,
        py.import("contextvars")?
            .getattr("ContextVar")?
            .call((CONTEXT_NAME,), Some(&keywords))?,
    )?;
    module.add_class::<TerminalEventIndex>()?;
    module.add_class::<TerminalEventClaim>()?;
    module.add_class::<TerminalEventIndexScope>()?;
    module.add_function(wrap_pyfunction!(current_terminal_event_index, module)?)?;
    module.add_function(wrap_pyfunction!(terminal_event_index_scope, module)?)
}
