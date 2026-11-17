//! Register the native column lineage functions and classes, and `sqb lineage`.

use pyo3::prelude::{Bound, PyModule, PyResult};

use crate::bindings::_helpers::lineage::lineage_command;
use sqlbuild_python_compile::lineage::main::register_column_lineage_graph::register_column_lineage_graph;
use sqlbuild_python_compile::lineage::main::register_project_lineage::register_project_lineage;
use sqlbuild_python_compile::lineage::main::register_relation_fingerprint::register_relation_fingerprint;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    register_column_lineage_graph(module)?;
    register_project_lineage(module)?;
    register_relation_fingerprint(module)?;
    lineage_command::register(module)
}
