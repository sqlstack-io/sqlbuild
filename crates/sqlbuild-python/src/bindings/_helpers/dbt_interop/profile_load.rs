//! Read the dbt project metadata and the selected profile output from the user's dbt YAML files.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_config::dbt_profile::main::dbt_profile_outputs::dbt_profile_outputs;
use sqlbuild_config::dbt_profile::main::dbt_project_metadata::dbt_project_metadata;
use sqlbuild_config::dbt_profile::main::dbt_yaml_mapping::dbt_yaml_mapping;
use sqlbuild_config::dbt_profile::main::select_dbt_profile_target::select_dbt_profile_target;
use sqlbuild_config::dbt_profile::models::{
    DbtProfileOutputs, DbtProjectMetadata, DbtYamlValue, LoadedDbtProfile,
};
use sqlbuild_host::entries::models::ThirdPartyHost;

use crate::bindings::_helpers::dbt_interop::profile::raised;
use crate::bindings::_helpers::dbt_interop::profile_connection::profile_error;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;

const DBT_PROJECT_FILE: &str = "dbt_project.yml";
const PROFILES_FILE: &str = "profiles.yml";
const DEFAULT_PROFILES_DIR: &str = "~/.dbt";

fn yaml_value(value: Option<&Bound<'_, PyAny>>) -> PyResult<DbtYamlValue> {
    let Some(value) = value.filter(|value| !value.is_none()) else {
        return Ok(DbtYamlValue::Null);
    };
    if value.is_instance_of::<PyString>() {
        return Ok(DbtYamlValue::Text(value.extract()?));
    }
    if let Some(mapping) = mapping(value.clone())? {
        return Ok(DbtYamlValue::Mapping {
            empty: mapping.is_empty(),
        });
    }
    Ok(DbtYamlValue::Other)
}

/// `value` as a dict when it is one.
fn mapping(value: Bound<'_, PyAny>) -> PyResult<Option<Bound<'_, PyDict>>> {
    if !value.is_instance_of::<PyDict>() {
        return Ok(None);
    }
    Ok(Some(value.cast_into::<PyDict>()?))
}

fn get<'py>(
    mapping: Option<&Bound<'py, PyDict>>,
    key: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    match mapping {
        Some(mapping) => mapping.get_item(key),
        None => Ok(None),
    }
}

/// The top-level mapping of one dbt config file; `None` when it holds no document.
fn yaml_mapping<'py>(file: &Bound<'py, PyAny>) -> PyResult<Option<Bound<'py, PyDict>>> {
    let py: Python<'py> = file.py();
    let display: String = file.str()?.extract()?;
    if !file.call_method0("exists")?.is_truthy()? {
        return Err(profile_error(
            py,
            format!("dbt config file not found: {display}"),
        ));
    }
    let document: Bound<'py, PyAny> = call_with(
        &host_entry(py, ThirdPartyHost::LoadDbtYaml)?,
        &[("file_path", file.clone())],
    )?;
    dbt_yaml_mapping(&display, &yaml_value(Some(&document))?).map_err(|error| raised(py, error))?;
    mapping(document)
}

/// The project name, profile name and target path of the dbt project in `project_dir`.
pub(crate) fn load_dbt_project_metadata(
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<DbtProjectMetadata> {
    let py: Python<'_> = project_dir.py();
    let file: Bound<'_, PyAny> = project_dir.call_method1("__truediv__", (DBT_PROJECT_FILE,))?;
    let payload: Option<Bound<'_, PyDict>> = yaml_mapping(&file)?;
    let values: [DbtYamlValue; 3] = [
        yaml_value(get(payload.as_ref(), "name")?.as_ref())?,
        yaml_value(get(payload.as_ref(), "profile")?.as_ref())?,
        yaml_value(get(payload.as_ref(), "target-path")?.as_ref())?,
    ];
    let [name, profile, target_path] = &values;
    dbt_project_metadata(
        &file.str()?.extract::<String>()?,
        (name, profile, target_path),
    )
    .map_err(|error| raised(py, error))
}

/// The `SelectedDbtProfileOutput` of `target_name`, or the profile default, from `profiles.yml`.
pub(crate) fn load_selected_dbt_profile_output<'py>(
    profiles_dir: &Bound<'py, PyAny>,
    profile_name: &str,
    target_name: Option<&str>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = profiles_dir.py();
    let file: Bound<'py, PyAny> = profiles_dir.call_method1("__truediv__", (PROFILES_FILE,))?;
    let payload: Option<Bound<'py, PyDict>> = yaml_mapping(&file)?;
    let profile: Option<Bound<'py, PyAny>> = get(payload.as_ref(), profile_name)?;
    let profile_mapping: Option<Bound<'py, PyDict>> = match &profile {
        Some(value) => mapping(value.clone())?,
        None => None,
    };
    let outputs: Option<Bound<'py, PyAny>> = get(profile_mapping.as_ref(), "outputs")?;
    let output_mapping: Option<Bound<'py, PyDict>> = match &outputs {
        Some(value) => mapping(value.clone())?,
        None => None,
    };
    let mut output_values: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut output_entries: Vec<(DbtYamlValue, DbtYamlValue)> = Vec::new();
    if let Some(mapping) = &output_mapping {
        for (name, output) in mapping.iter() {
            output_entries.push((yaml_value(Some(&name))?, yaml_value(Some(&output))?));
            output_values.push(output);
        }
    }
    let loaded: LoadedDbtProfile = LoadedDbtProfile {
        profile: yaml_value(profile.as_ref())?,
        outputs: yaml_value(outputs.as_ref())?,
        output_entries,
        target: yaml_value(get(profile_mapping.as_ref(), "target")?.as_ref())?,
    };
    let validated: DbtProfileOutputs =
        dbt_profile_outputs(&file.str()?.extract::<String>()?, profile_name, &loaded)
            .map_err(|error| raised(py, error))?;
    let (target, index): (String, usize) =
        select_dbt_profile_target(&validated, target_name).map_err(|error| raised(py, error))?;
    new_record_with(
        py,
        RecordId::SelectedDbtProfileOutput,
        &[
            ("profile_name", PyString::new(py, profile_name).into_any()),
            ("target_name", PyString::new(py, &target).into_any()),
            ("output", output_values[index].clone()),
        ],
    )
}

/// dbt's default profiles directory.
pub(crate) fn default_profiles_dir(py: Python<'_>) -> PyResult<Bound<'_, PyAny>> {
    imported(py, "pathlib", "Path")?
        .call1((DEFAULT_PROFILES_DIR,))?
        .call_method0("expanduser")
}
