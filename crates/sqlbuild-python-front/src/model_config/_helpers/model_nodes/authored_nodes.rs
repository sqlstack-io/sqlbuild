//! Python objects read in place as authored header values.

use crate::model_config::models::PyNode;
use pyo3::prelude::{Bound, PyAny};

pub(crate) fn entry_nodes<'py>(
    (key, value): (Bound<'py, PyAny>, Bound<'py, PyAny>),
) -> (PyNode<'py>, PyNode<'py>) {
    (PyNode(key), PyNode(value))
}
