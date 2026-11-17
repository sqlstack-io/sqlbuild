//! How many Python nodes a project declares.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The discovered inputs' Python node functions, each kind of which keeps node results.
const PYTHON_NODE_FIELDS: [&str; 4] = [
    "loader_functions",
    "task_functions",
    "asset_functions",
    "check_functions",
];

/// How many loaders, tasks, assets and checks the project declares in Python.
pub fn python_node_count(discovered: &Bound<'_, PyAny>) -> PyResult<usize> {
    PYTHON_NODE_FIELDS
        .iter()
        .map(|field| native_field(discovered, field)?.len())
        .sum()
}
