//! The Python object of one config value: new builtins, and opaque values as themselves.

use crate::model_config::_helpers::config::config_nodes;
use crate::model_config::types::PyConfigNode;
use pyo3::prelude::Bound;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The Python object of one config value: new builtins, and opaque values as themselves.
#[inline]
pub fn config_object<'py>(py: Python<'py>, node: &PyConfigNode) -> PyResult<Bound<'py, PyAny>> {
    config_nodes::config_object(py, node)
}
