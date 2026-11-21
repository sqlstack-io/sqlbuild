//! How many Python nodes a project declares.

use crate::compile_driver::_helpers::setup::node_results;
use pyo3::prelude::{Bound, PyAny, PyResult};

/// How many loaders, assets and checks the project declares in Python.
pub fn python_node_count(discovered: &Bound<'_, PyAny>) -> PyResult<usize> {
    node_results::python_node_count(discovered)
}
