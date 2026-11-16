//! Decorated model lifecycle hook functions under hooks/python/.

use crate::discovery::_helpers::python_files::imported_python_nodes;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// Decorated model lifecycle hook functions under hooks/python/.
#[inline]
pub fn discover_hook_functions<'py>(
    project_dir: Bound<'py, PyAny>,
    providers: Option<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    imported_python_nodes::discover_hook_functions(project_dir, providers)
}
