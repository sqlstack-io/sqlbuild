//! Resolve a `source = "dbt_profile"` connection through the user's dbt profile.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::text::main::python_strip::python_strip;
use sqlbuild_host::entries::models::ThirdPartyHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use crate::bindings::_helpers::dbt_interop::cli::resolved_path;
use crate::bindings::_helpers::dbt_interop::profile::normalize_dbt_profile_connection;
use crate::bindings::_helpers::dbt_interop::profile_load::{
    default_profiles_dir, load_dbt_project_metadata, load_selected_dbt_profile_output,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const DBT_PROFILE_SOURCE: &str = "dbt_profile";
const PROFILE_CONNECTION_OPTIONS: (&[&str], &[&str]) = (
    &[
        "raw_config",
        "project_dir",
        "adapter_name",
        "project_config",
    ],
    &["cli_vars"],
);
const NON_EMPTY_MESSAGE: &str = "dbt_profile connection values must be non-empty strings";

pub(crate) fn profile_error(py: Python<'_>, message: String) -> PyErr {
    host_error(py, ThirdPartyHost::DbtProfileError, &message)
}

/// A raw connection value as a stripped non-empty string, or `None` when absent.
fn optional_str(config: &Bound<'_, PyDict>, key: &str) -> PyResult<Option<String>> {
    let py: Python<'_> = config.py();
    let Some(value) = config.get_item(key)? else {
        return Ok(None);
    };
    if value.is_none() {
        return Ok(None);
    }
    let Ok(value) = value.cast_into::<PyString>() else {
        return Err(profile_error(py, NON_EMPTY_MESSAGE.to_owned()));
    };
    let text: String = value.extract()?;
    let stripped: &str = python_strip(&text);
    if stripped.is_empty() {
        return Err(profile_error(py, NON_EMPTY_MESSAGE.to_owned()));
    }
    Ok(Some(stripped.to_owned()))
}

fn or_config(value: Option<String>, fallback: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_some() || fallback.is_none() {
        return Ok(value);
    }
    Ok(Some(fallback.extract()?))
}

/// The dbt profile reference rendered and normalized into SQLBuild adapter config.
fn profile_connection<'py>(
    sqlbuild_project_dir: &Bound<'py, PyAny>,
    (dbt_project_dir, profiles_dir): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    (profile_name, target_name): (Option<String>, Option<String>),
    cli_vars: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = sqlbuild_project_dir.py();
    let dbt_project_dir: Bound<'py, PyAny> = if dbt_project_dir.is_none() {
        sqlbuild_project_dir.clone()
    } else {
        dbt_project_dir
    };
    let profiles_dir: Bound<'py, PyAny> = if profiles_dir.is_truthy()? {
        profiles_dir
    } else {
        default_profiles_dir(py)?
    };
    let profile_name: String = match profile_name {
        Some(name) => name,
        None => load_dbt_project_metadata(&dbt_project_dir)?.profile_name,
    };
    let selected: Bound<'py, PyAny> =
        load_selected_dbt_profile_output(&profiles_dir, &profile_name, target_name.as_deref())?;
    let resolved: Bound<'py, PyAny> = call_with(
        &host_entry(py, ThirdPartyHost::RenderSelectedDbtProfileOutput)?,
        &[
            ("selected", selected),
            ("project_dir", dbt_project_dir),
            ("profiles_dir", profiles_dir),
            ("cli_vars", cli_vars),
        ],
    )?;
    normalize_dbt_profile_connection(py, resolved)
}

/// Resolve a raw connection config if it references a dbt profile.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn resolve_raw_dbt_profile_connection<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_raw_dbt_profile_connection",
        options,
        PROFILE_CONNECTION_OPTIONS,
    )?;
    let raw_config: Bound<'py, PyDict> = arguments.value("raw_config")?.cast_into()?;
    let project_dir: Bound<'py, PyAny> = arguments.value("project_dir")?;
    let adapter_name: Bound<'py, PyAny> = arguments.value("adapter_name")?;
    let project_config: Bound<'py, PyAny> = arguments.value("project_config")?;
    let cli_vars: Bound<'py, PyAny> = arguments.value("cli_vars")?;
    let is_profile: bool = match raw_config.get_item("source")? {
        Some(source) => source.eq(DBT_PROFILE_SOURCE)?,
        None => false,
    };
    if !is_profile {
        return Ok(py.None().into_bound(py));
    }
    let dbt_config: Bound<'py, PyAny> = native_field(&project_config, "dbt")?;
    let dbt_project_dir: Bound<'py, PyAny> = resolved_path(
        &project_dir,
        or_config(
            optional_str(&raw_config, "project_dir")?,
            &native_field(&dbt_config, "project_dir")?,
        )?,
    )?;
    let profiles_dir: Bound<'py, PyAny> = resolved_path(
        &project_dir,
        or_config(
            optional_str(&raw_config, "profiles_dir")?,
            &native_field(&dbt_config, "profiles_dir")?,
        )?,
    )?;
    let profile_name: Option<String> = optional_str(&raw_config, "profile")?;
    let target_name: Option<String> = or_config(
        optional_str(&raw_config, "target")?,
        &native_field(&dbt_config, "target")?,
    )?;
    let cli_vars: Bound<'py, PyAny> = if cli_vars.is_none() {
        PyDict::new(py).into_any()
    } else {
        cli_vars
    };
    let resolved: Bound<'py, PyAny> = profile_connection(
        &project_dir,
        (dbt_project_dir, profiles_dir),
        (profile_name, target_name),
        cli_vars,
    )?;
    let resolved_adapter: Bound<'py, PyAny> = native_field(&resolved, "adapter")?;
    if !resolved_adapter.eq(&adapter_name)? {
        return Err(profile_error(
            py,
            format!(
                "dbt profile adapter type does not match SQLBuild project adapter: dbt={}, sqlbuild={}",
                resolved_adapter.str()?,
                adapter_name.str()?
            ),
        ));
    }
    Ok(resolved)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        resolve_raw_dbt_profile_connection,
        module
    )?)
}
