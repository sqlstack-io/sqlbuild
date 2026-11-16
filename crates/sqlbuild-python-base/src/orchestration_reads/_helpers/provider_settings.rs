//! A provider class's declared settings and whether the environment sets its required secrets.

use crate::orchestration_reads::models::ProviderState;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use serde_json::{Value, json};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::boundary::main::call_with::call_with;
use crate::boundary::main::imported::imported;
use crate::native_host::constants::ALIAS_PREFIX_TARGET;
use crate::native_host::constants::ALL_PREFIX_TARGET;
use crate::native_host::main::host_entry::host_entry;
use crate::records::main::fields::mapping_get_or::mapping_get_or;

const VALIDATION_ERROR: (&str, &str) = ("pydantic", "ValidationError");
const MISSING_ERROR_TYPE: &str = "missing";

/// The class's description and declared setting fields, read without any setting value.
pub(crate) fn provider_class_settings(provider_class: &Bound<'_, PyAny>) -> PyResult<Value> {
    let py: Python<'_> = provider_class.py();
    let settings: Bound<'_, PyAny> = call_with(
        &host_entry(py, UserCodeHost::ProviderSettings)?,
        &[("provider_class", provider_class.clone())],
    )?;
    let text: String = imported(py, "json", "dumps")?
        .call1((settings,))?
        .extract()?;
    serde_json::from_str(&text).map_err(|error| PyValueError::new_err(error.to_string()))
}

/// Declared fields with the environment variable each is read from, which pydantic-settings derives from the prefix unless an alias names it.
pub(crate) fn setting_rows(class: &Value) -> Vec<Value> {
    let prefix = class["env_prefix"].as_str().unwrap_or_default();
    let case_sensitive = class["case_sensitive"].as_bool().unwrap_or_default();
    let target = class["env_prefix_target"].as_str();
    let alias_prefix = match target {
        Some(ALIAS_PREFIX_TARGET | ALL_PREFIX_TARGET) => prefix,
        _ => "",
    };
    let name_prefix = match target {
        Some(ALIAS_PREFIX_TARGET) => "",
        _ => prefix,
    };
    let mut rows: Vec<Value> = Vec::new();
    for field in class["fields"].as_array().into_iter().flatten() {
        let name = field["name"].as_str().unwrap_or_default();
        let type_name = field["type"].as_str().unwrap_or_default();
        let env = field["alias"].as_str().map_or_else(
            || format!("{name_prefix}{name}"),
            |alias| format!("{alias_prefix}{alias}"),
        );
        rows.push(json!({
            "name": name,
            "alias": field["alias"],
            "type": type_name,
            "required": field["required"],
            "secret": type_name.contains("Secret"),
            "env": if case_sensitive { env } else { env.to_uppercase() },
            "description": field["description"],
        }));
    }
    rows
}

/// The environment variables of the class's secret settings.
pub(crate) fn secret_variables(provider_class: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    Ok(setting_rows(&provider_class_settings(provider_class)?)
        .iter()
        .filter(|row| row["secret"].as_bool().unwrap_or_default())
        .filter_map(|row| row["env"].as_str().map(str::to_owned))
        .collect())
}

/// Instantiate the class, telling unset required secrets apart from other invalid settings.
pub(crate) fn provider_state<'py>(
    provider_class: &Bound<'py, PyAny>,
) -> PyResult<ProviderState<'py>> {
    let py: Python<'py> = provider_class.py();
    let error: PyErr = match provider_class.call0() {
        Ok(instance) => return Ok(ProviderState::Ready(instance)),
        Err(error) => error,
    };
    if !error
        .value(py)
        .is_instance(&imported(py, VALIDATION_ERROR.0, VALIDATION_ERROR.1)?)?
    {
        return Err(error);
    }
    let rows: Vec<Value> = setting_rows(&provider_class_settings(provider_class)?);
    let options: Bound<'py, PyDict> = PyDict::new(py);
    options.set_item("include_input", false)?;
    let mut missing: Vec<String> = Vec::new();
    for item in error
        .value(py)
        .call_method("errors", (), Some(&options))?
        .try_iter()?
    {
        let item: Bound<'py, PyAny> = item?;
        let kind: String = mapping_get_or(&item, "type", "")?.str()?.extract()?;
        let location: Vec<String> = mapping_get_or(&item, "loc", ())?
            .try_iter()?
            .map(|part| part?.str()?.extract::<String>())
            .collect::<PyResult<Vec<String>>>()?;
        let secret: Option<&str> = match location.as_slice() {
            [field] if kind == MISSING_ERROR_TYPE => secret_env(&rows, field),
            _ => None,
        };
        let Some(env) = secret else {
            return Ok(ProviderState::Invalid(error));
        };
        missing.push(env.to_owned());
    }
    if missing.is_empty() {
        return Ok(ProviderState::Invalid(error));
    }
    missing.sort();
    missing.dedup();
    Ok(ProviderState::MissingSecrets(missing))
}

/// The environment variables of the class's required secrets the environment leaves unset; an invalid setting is an error.
pub(crate) fn missing_secrets(provider_class: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    match provider_state(provider_class)? {
        ProviderState::Ready(_) => Ok(Vec::new()),
        ProviderState::MissingSecrets(missing) => Ok(missing),
        ProviderState::Invalid(error) => Err(error),
    }
}

/// The environment variable of the secret setting pydantic reports as `field`, by its name or alias.
fn secret_env<'a>(rows: &'a [Value], field: &str) -> Option<&'a str> {
    rows.iter()
        .filter(|row| row["secret"].as_bool().unwrap_or_default())
        .find(|row| {
            row["name"].as_str() == Some(field)
                || row["alias"].as_str() == Some(field)
                || row["env"]
                    .as_str()
                    .is_some_and(|env| env.eq_ignore_ascii_case(field))
        })
        .and_then(|row| row["env"].as_str())
}
