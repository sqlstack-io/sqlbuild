//! The model's project graph row: its key, dependencies, tags and folder.

use crate::compiled_views::_helpers::compiled_models::compiled_model_view;
use crate::compiled_views::models::NativeCompiledModel;
use crate::graph::models::ResourceRow;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;

/// The model's project graph row: its key, dependencies, tags and folder.
#[inline]
pub fn model_graph_row(model: &NativeCompiledModel, py: Python<'_>) -> PyResult<ResourceRow> {
    compiled_model_view::model_graph_row(model, py)
}
