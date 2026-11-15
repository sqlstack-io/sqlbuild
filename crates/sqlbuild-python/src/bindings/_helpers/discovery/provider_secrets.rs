//! A provider class's setting fields and which of its secret settings the environment leaves unset.

use std::cell::Cell;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods};
use serde_json::{Value, json};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::boundary::python_calls::{call_with, imported};
use crate::bindings::_helpers::compiled_project::values::plain_value;
use crate::bindings::_helpers::discovery::constants::{ALIAS_PREFIX_TARGET, ALL_PREFIX_TARGET};
use crate::bindings::_helpers::discovery::imported_modules::{discovery_error, shim};
use crate::bindings::_helpers::records::fields::{field, mapping_get_or};
use crate::bindings::_helpers::records::ids::ErrorId;

const VALIDATION_ERROR: (&str, &str) = ("pydantic", "ValidationError");
const MISSING_ERROR_TYPE: &str = "missing";
/// Discovered collections of sinks that run with every command.
const SINK_COLLECTIONS: [&str; 2] = ["event_exporters", "command_output_sinks"];

thread_local! {
    /// Whether the invocation running on this thread passed `--allow-missing-secrets`.
    static INVOCATION_ALLOWS_MISSING_SECRETS: Cell<bool> = const { Cell::new(false) };
}

/// `--allow-missing-secrets` for one invocation, restored when the invocation ends.
pub(crate) struct MissingSecretsScope {
    previous: bool,
}

impl MissingSecretsScope {
    pub(crate) fn enter(allowed: bool) -> Self {
        Self {
            previous: INVOCATION_ALLOWS_MISSING_SECRETS.replace(allowed),
        }
    }
}

impl Drop for MissingSecretsScope {
    fn drop(&mut self) {
        INVOCATION_ALLOWS_MISSING_SECRETS.set(self.previous);
    }
}

/// Whether the running invocation passed `--allow-missing-secrets`.
pub(crate) fn invocation_allows_missing_secrets() -> bool {
    INVOCATION_ALLOWS_MISSING_SECRETS.get()
}

/// What discovery does when a provider's required secrets are unset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SecretPolicy {
    /// Fail with D012: static commands by default.
    Require,
    /// Continue and list the providers on stderr: static commands that opted out.
    Allow,
    /// Continue silently; the command checks the providers it uses before running anything.
    Defer,
}

impl SecretPolicy {
    /// The keyword value that selects the policy.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Require => "require",
            Self::Allow => "allow",
            Self::Defer => "defer",
        }
    }

    pub(crate) fn parse(value: &str) -> PyResult<Self> {
        [Self::Require, Self::Allow, Self::Defer]
            .into_iter()
            .find(|policy| policy.name() == value)
            .ok_or_else(|| PyValueError::new_err(format!("unknown secret policy '{value}'")))
    }
}

/// How a provider class instantiates from the environment.
pub(crate) enum ProviderState<'py> {
    Ready(Bound<'py, PyAny>),
    /// Only required secret settings are unset: their environment variables, sorted.
    MissingSecrets(Vec<String>),
    /// Another setting is invalid: pydantic's validation error.
    Invalid(PyErr),
}

/// The class's description and declared setting fields, read without any setting value.
pub(crate) fn provider_class_settings(provider_class: &Bound<'_, PyAny>) -> PyResult<Value> {
    let py: Python<'_> = provider_class.py();
    plain_value(&call_with(
        &shim(py, UserCodeHost::ProviderSettings)?,
        &[("provider_class", provider_class.clone())],
    )?)
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

/// `(provider, consumer)` for each usage, named by the usage's own `consumer_name` or else by `consumer`.
pub(crate) fn provider_uses(
    usages: &Bound<'_, PyAny>,
    consumer: Option<&str>,
) -> PyResult<Vec<(String, String)>> {
    let mut uses: Vec<(String, String)> = Vec::new();
    for usage in usages.try_iter()? {
        let usage: Bound<'_, PyAny> = usage?;
        let provider: String = field(&usage, "provider_name")?.extract()?;
        let name: String = match consumer {
            Some(name) => name.to_owned(),
            None => field(&usage, "consumer_name")?.extract()?,
        };
        uses.push((provider, name));
    }
    Ok(uses)
}

/// `(provider, consumer)` for each provider the named consumers, such as checks or loaders, inject.
pub(crate) fn consumer_uses(consumers: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut uses: Vec<(String, String)> = Vec::new();
    for consumer in consumers.try_iter()? {
        let consumer: Bound<'_, PyAny> = consumer?;
        let name: String = field(&consumer, "name")?.extract()?;
        uses.extend(provider_uses(
            &field(&consumer, "provider_usages")?,
            Some(&name),
        )?);
    }
    Ok(uses)
}

/// The providers the project's lifecycle and command-output sinks use, which run with every command.
pub(crate) fn sink_uses(discovered: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let mut uses: Vec<(String, String)> = Vec::new();
    for collection in SINK_COLLECTIONS {
        if let Some(sinks) = discovered.getattr_opt(collection)? {
            uses.extend(consumer_uses(&sinks)?);
        }
    }
    Ok(uses)
}

/// Fail before anything runs when a provider in `uses` lacks required secrets, listing each with what needs it.
pub(crate) fn require_used_secrets(
    discovered: &Bound<'_, PyAny>,
    uses: &[(String, String)],
) -> PyResult<()> {
    let py: Python<'_> = discovered.py();
    let mut consumers: Vec<(String, Vec<String>)> = Vec::new();
    for (provider, consumer) in uses {
        match consumers.iter_mut().find(|(name, _)| name == provider) {
            Some((_, names)) if !names.contains(consumer) => names.push(consumer.clone()),
            Some(_) => {}
            None => consumers.push((provider.clone(), vec![consumer.clone()])),
        }
    }
    let mut rows: Vec<[String; 3]> = Vec::new();
    for provider in field(discovered, "providers")?.try_iter()? {
        let provider: Bound<'_, PyAny> = provider?;
        let name: String = field(&provider, "name")?.extract()?;
        let Some((_, names)) = consumers.iter().find(|(used, _)| *used == name) else {
            continue;
        };
        let needed_by: String = match names.as_slice() {
            [only] => only.clone(),
            [first, rest @ ..] => format!("{first}, +{}", rest.len()),
            [] => String::new(),
        };
        for variable in missing_secrets(&field(&provider, "provider_class")?)? {
            rows.push([name.clone(), variable, format!("({needed_by})")]);
        }
    }
    if rows.is_empty() {
        return Ok(());
    }
    let widths: [usize; 2] = [0, 1].map(|column| {
        rows.iter()
            .map(|row| row[column].len())
            .max()
            .unwrap_or_default()
    });
    let lines: Vec<String> = rows
        .iter()
        .map(|[provider, variable, needed_by]| {
            format!(
                "  {provider:<provider_width$}  {variable:<variable_width$}  {needed_by}",
                provider_width = widths[0],
                variable_width = widths[1],
            )
        })
        .collect();
    let error: PyErr = discovery_error(
        py,
        ErrorId::ProviderDiscovery,
        format!(
            "selected nodes need secrets that are missing:\n{}\nnothing was run",
            lines.join("\n")
        ),
    );
    error
        .value(py)
        .setattr("help", "set each variable, or narrow the selection")?;
    Err(error)
}
