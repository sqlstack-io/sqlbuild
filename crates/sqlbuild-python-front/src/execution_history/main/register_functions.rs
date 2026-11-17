//! Register the functions functions and classes.

use crate::execution_history::_helpers::functions::append_and_project;
use crate::execution_history::_helpers::functions::canonical_event_content;
use crate::execution_history::_helpers::functions::canonical_event_id;
use crate::execution_history::_helpers::functions::project_execution_runs;
use crate::execution_history::_helpers::functions::validate_history_event_filter;
use crate::execution_history::_helpers::functions::validate_history_page;
use crate::execution_history::_helpers::functions::validate_history_page_limit;
use crate::execution_history::_helpers::functions::validate_history_run_filter;
use crate::execution_history::_helpers::functions::validate_stored_history_event;
use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult, wrap_pyfunction};

/// Register the functions functions and classes.
pub fn register_functions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(canonical_event_id, module)?)?;
    module.add_function(wrap_pyfunction!(canonical_event_content, module)?)?;
    module.add_function(wrap_pyfunction!(project_execution_runs, module)?)?;
    module.add_function(wrap_pyfunction!(validate_history_page_limit, module)?)?;
    module.add_function(wrap_pyfunction!(append_and_project, module)?)?;
    module.add_function(wrap_pyfunction!(validate_stored_history_event, module)?)?;
    module.add_function(wrap_pyfunction!(validate_history_event_filter, module)?)?;
    module.add_function(wrap_pyfunction!(validate_history_run_filter, module)?)?;
    module.add_function(wrap_pyfunction!(validate_history_page, module)?)
}
