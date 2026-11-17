//! Which models `[defaults]`, `[path_defaults]` or a reusable model schema describe.

use crate::model_config::main::config::config_node::config_node;
use crate::model_config::main::config::config_object::config_object;
use crate::model_config::main::model_nodes::raised_config_error::raised_config_error;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyListMethods, PyStringMethods};
use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_model_config::config_values::main::config_entry::config_entry;
use sqlbuild_model_config::config_values::main::layered_config::layered_config;
use sqlbuild_model_config::path_defaults::main::select_path_default::select_path_default;
use sqlbuild_model_config::path_defaults::models::PathDefaultChoice;
use sqlbuild_model_config::types::AuthoredNode;

use crate::model_config::_helpers::config::config_nodes::{
    PyConfigEntries, config_entries, is_text_node,
};
use crate::model_config::_helpers::model_nodes::project_defaults::project_defaults_mapping;
use crate::model_config::types::PyConfigNode;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const DESCRIPTION_KEY: &str = "description";
const MODEL_SCHEMA_KEY: &str = "model_schema";

/// Whether `value` is a string with non-whitespace text.
fn non_empty_text(value: Option<&PyConfigNode>) -> bool {
    value
        .filter(|value| is_text_node(value))
        .and_then(|value| value.with_text(|text| !python_strip(text).is_empty()))
        .unwrap_or(false)
}

/// The file path of each model whose layered config or model schema gives it a description.
pub(crate) fn externally_described_models<'py>(
    inputs: Bound<'py, PyAny>,
    model_schemas: Bound<'py, PyDict>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py = inputs.py();
    let project_config: Bound<'py, PyAny> = native_field(&inputs, "project_config")?;
    let defaults: PyConfigEntries = config_entries(&project_defaults_mapping(&native_field(
        &project_config,
        "defaults",
    )?)?)?;
    let path_defaults: Bound<'py, PyDict> =
        native_field(&project_config, "path_defaults")?.downcast_into()?;
    let path_keys: Vec<String> = path_defaults
        .keys()
        .iter()
        .map(|key| key.extract::<String>())
        .collect::<PyResult<_>>()?;
    let text = |item: &PyConfigNode| -> PyResult<String> {
        Ok(config_object(py, item)?
            .str()?
            .to_string_lossy()
            .into_owned())
    };
    let mut layers: HashMap<String, PyConfigEntries> = HashMap::new();
    let mut described: Vec<Bound<'py, PyAny>> = Vec::new();
    for model_file in native_field(&inputs, "model_files")?.try_iter()? {
        let model_file: Bound<'py, PyAny> = model_file?;
        let relative_path: String = native_field(&model_file, "relative_path")?
            .str()?
            .extract()?;
        let mut values: PyConfigEntries = defaults.clone();
        match select_path_default(&relative_path, &path_keys) {
            PathDefaultChoice::Selected(Some(key)) => {
                if !layers.contains_key(&key) {
                    let layer: Bound<'py, PyDict> =
                        path_defaults.as_any().get_item(&key)?.downcast_into()?;
                    layers.insert(key.clone(), config_entries(&layer)?);
                }
                values = layered_config(&values, &layers[&key], &text)?.entries;
            }
            PathDefaultChoice::Selected(None) => {}
            PathDefaultChoice::Conflict(error) => return Err(raised_config_error(py, error)),
        }
        let header: Bound<'py, PyDict> =
            native_field(&model_file, "header_values")?.downcast_into()?;
        values = layered_config(&values, &config_entries(&header)?, &text)?.entries;
        let schema_description: Option<PyConfigNode> = match config_entry(&values, MODEL_SCHEMA_KEY)
        {
            Some(name) if is_text_node(name) => model_schemas
                .get_item(config_object(py, name)?)?
                .map(|schema| schema.getattr(DESCRIPTION_KEY))
                .transpose()?
                .map(|description| config_node(&description))
                .transpose()?,
            _ => None,
        };
        if non_empty_text(config_entry(&values, DESCRIPTION_KEY))
            || non_empty_text(schema_description.as_ref())
        {
            described.push(native_field(&model_file, "file_path")?);
        }
    }
    Ok(described)
}
