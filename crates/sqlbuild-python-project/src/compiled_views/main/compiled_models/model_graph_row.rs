//! The model's project graph row: its key, dependencies, tags and folder.

use crate::compiled_views::_helpers::compiled_models::compiled_model_view::CompiledRecord;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::MODEL_RESOURCE;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::TAGS_KEY;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::VALUES_ATTRIBUTE;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::compiled_of;
use crate::compiled_views::_helpers::compiled_models::compiled_model_view::model_name;
use crate::compiled_views::models::NativeCompiledModel;
use crate::graph::models::ResourceRow;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use sqlbuild_python_base::records::main::fields::field::field as native_field;

/// The model's project graph row: its key, dependencies, tags and folder.
pub fn model_graph_row(model: &NativeCompiledModel, py: Python<'_>) -> PyResult<ResourceRow> {
    model.read(py, |record, py| {
        let compiled: &CompiledRecord = compiled_of(record)?;
        let values: Bound<'_, PyAny> = native_field(record.host.config.bind(py), VALUES_ATTRIBUTE)?;
        let mut tags: Vec<String> = Vec::new();
        if let Some(value) = values.downcast::<PyDict>()?.get_item(TAGS_KEY)?
            && (value.is_instance_of::<pyo3::types::PyList>()
                || value.is_instance_of::<pyo3::types::PyTuple>())
        {
            for tag in value.try_iter()? {
                tags.push(tag?.str()?.extract()?);
            }
        }
        let folder: String = record.host.model_file.get().folder(py)?;
        Ok(ResourceRow {
            key: (MODEL_RESOURCE.to_owned(), model_name(record)),
            deps: compiled.deps.clone(),
            tags,
            folder: Some(folder),
        })
    })
}
