//! Discovered contract objects converted to authoritative fixture schemas.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::types::{PyDict, PyDictMethods, PyListMethods, PyStringMethods};
use sqlbuild_model_config::path_defaults::main::select_path_default::select_path_default;
use sqlbuild_model_config::path_defaults::models::PathDefaultChoice;
use sqlbuild_rules::lint_text::models::FixtureFormatRelation;
use sqlbuild_rules::lint_text::types::FixtureFormatColumn;
use std::collections::HashMap;
use std::path::Path;

use crate::bindings::_helpers::discovery::model_file_record::NativeModelFile;
use crate::bindings::_helpers::model_config::header_columns::schema_columns;
use crate::bindings::_helpers::model_config::model_config_builder::raised_config_error;
use sqlbuild_python_base::records::main::fields::field::field;

pub(crate) fn columns(columns: &Bound<'_, PyAny>) -> PyResult<Vec<FixtureFormatColumn>> {
    columns
        .try_iter()?
        .map(|column| {
            let column = column?;
            Ok((
                field(&column, "name")?.extract()?,
                field(&column, "type")?.extract()?,
                field(&column, "nullable")?.extract()?,
            ))
        })
        .collect()
}

/// The parts of a discovered model file the fixture schemas read.
struct ModelFileView<'py> {
    file_path: Bound<'py, PyAny>,
    relative_path: Bound<'py, PyAny>,
    header_values: Bound<'py, PyDict>,
    header_column_locations: Bound<'py, PyDict>,
}

/// A native model file's parts read in place; any other model file's by attribute.
fn model_file_view<'py>(model: &Bound<'py, PyAny>) -> PyResult<ModelFileView<'py>> {
    let py = model.py();
    if let Ok(native) = model.cast::<NativeModelFile>() {
        let native: &NativeModelFile = native.get();
        let (file_path, relative_path) = native.path_objects(py)?;
        return Ok(ModelFileView {
            file_path,
            relative_path,
            header_values: native.header_value_dict(py)?,
            header_column_locations: native.header_locations(py)?,
        });
    }
    Ok(ModelFileView {
        file_path: field(model, "file_path")?,
        relative_path: field(model, "relative_path")?,
        header_values: field(model, "header_values")?.cast_into::<PyDict>()?,
        header_column_locations: field(model, "header_column_locations")?.cast_into::<PyDict>()?,
    })
}

/// `PurePath(file_name).stem`: the final component without its last suffix.
fn file_stem(file_name: &str) -> String {
    Path::new(file_name)
        .file_stem()
        .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
}

pub(crate) fn relations(
    inputs: &Bound<'_, PyAny>,
) -> PyResult<HashMap<String, FixtureFormatRelation>> {
    let py = inputs.py();
    let config = field(inputs, "project_config")?;
    let default_contract: Option<String> =
        field(&field(&config, "defaults")?, "contract")?.extract()?;
    let defaults = field(&config, "path_defaults")?.cast_into::<PyDict>()?;
    let keys: Vec<String> = defaults
        .keys()
        .iter()
        .map(|key| key.extract())
        .collect::<PyResult<_>>()?;
    let mut result: HashMap<String, FixtureFormatRelation> = HashMap::new();
    for model in field(inputs, "model_files")?.try_iter()? {
        let model = model?;
        let file: ModelFileView<'_> = model_file_view(&model)?;
        let path = file.relative_path.str()?.to_str()?.to_owned();
        let mut contract = default_contract.clone();
        match select_path_default(&path, &keys) {
            PathDefaultChoice::Selected(Some(key)) => {
                let values = defaults
                    .get_item(key)?
                    .ok_or_else(|| {
                        pyo3::exceptions::PyKeyError::new_err("selected path default is absent")
                    })?
                    .cast_into::<PyDict>()?;
                if let Some(value) = values.get_item("contract")? {
                    contract = value.extract()?;
                }
            }
            PathDefaultChoice::Selected(None) => {}
            PathDefaultChoice::Conflict(error) => return Err(raised_config_error(py, error)),
        }
        let header = &file.header_values;
        if let Some(value) = header.get_item("contract")? {
            contract = value.extract()?;
        }
        if contract.as_deref() != Some("enforced") {
            continue;
        }
        let file_name = file.file_path.str()?.to_str()?.to_owned();
        let raw_columns = header
            .get_item("columns")?
            .unwrap_or_else(|| py.None().into_bound(py));
        let parsed = schema_columns(
            &raw_columns,
            (&file_name, "model"),
            (false, true, false),
            &file.header_column_locations,
        )?;
        let columns = columns(parsed.as_any())?;
        if !columns.is_empty() {
            result.insert(
                format!("__ref__{}", file_stem(&file_name)),
                FixtureFormatRelation { columns },
            );
        }
    }
    for source_file in field(inputs, "source_files")?.try_iter()? {
        for source in field(&source_file?, "source_entries")?.try_iter()? {
            let source = source?;
            let contract: Option<String> = field(&source, "contract")?.extract()?;
            let contract = contract
                .filter(|value| !value.is_empty())
                .or_else(|| default_contract.clone());
            if contract.as_deref() != Some("enforced") {
                continue;
            }
            result.insert(
                format!("__source__{}", field(&source, "name")?.extract::<String>()?),
                FixtureFormatRelation {
                    columns: columns(&field(&source, "columns")?)?,
                },
            );
        }
    }
    for schema_file in field(inputs, "schema_files")?.try_iter()? {
        for seed in field(&schema_file?, "seed_entries")?.try_iter()? {
            let seed = seed?;
            let columns = columns(&field(&seed, "columns")?)?;
            if !columns.is_empty() {
                result.insert(
                    format!("__seed__{}", field(&seed, "name")?.extract::<String>()?),
                    FixtureFormatRelation { columns },
                );
            }
        }
    }
    Ok(result)
}
