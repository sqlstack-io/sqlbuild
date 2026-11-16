//! dbt profile normalization and the project config `sqb dbt init` writes.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_config::dbt_profile::errors::DbtProfileError;
use sqlbuild_config::dbt_profile::main::dbt_init_project_toml::dbt_init_project_toml;
use sqlbuild_config::dbt_profile::main::normalize_dbt_profile::normalize_dbt_profile;
use sqlbuild_config::dbt_profile::models::{
    ConnectionKey, ConnectionSource, DbtInitConfig, NormalizedProfile, ProfileEntry, ProfileValue,
};
use sqlbuild_host::entries::models::ThirdPartyHost;
use sqlbuild_python_base::native_host::main::host_error::host_error;

use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::RecordId;

const INIT_TOML_OPTIONS: (&[&str], &[&str]) = (
    &[
        "project_name",
        "adapter",
        "target_name",
        "profile_name",
        "dbt_project_dir",
        "profiles_dir",
        "target_path",
        "target_schema",
        "target_database",
    ],
    &[],
);

pub(crate) fn raised(py: Python<'_>, error: DbtProfileError) -> PyErr {
    host_error(py, ThirdPartyHost::DbtProfileError, &error.0)
}

fn profile_value(value: &Bound<'_, PyAny>) -> PyResult<ProfileValue> {
    if value.is_none() {
        return Ok(ProfileValue::Null);
    }
    if value.is_instance_of::<PyBool>() {
        return Ok(ProfileValue::Bool(value.is_truthy()?));
    }
    if value.is_instance_of::<PyString>() {
        return Ok(ProfileValue::Text(value.extract()?));
    }
    Ok(ProfileValue::Other)
}

fn optional_text<'py>(py: Python<'py>, value: Option<&str>) -> Bound<'py, PyAny> {
    match value {
        Some(text) => PyString::new(py, text).into_any(),
        None => py.None().into_bound(py),
    }
}

/// `NormalizedDbtProfileConnection` with the connection built from `output`'s values.
fn normalized_connection<'py>(
    output: &Bound<'py, PyDict>,
    profile: &NormalizedProfile,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = output.py();
    let items: Vec<(Bound<'py, PyAny>, Bound<'py, PyAny>)> = output.iter().collect();
    let connection: Bound<'py, PyDict> = PyDict::new(py);
    for (key, source) in &profile.connection {
        let key: Bound<'py, PyAny> = match key {
            ConnectionKey::Name(name) => PyString::new(py, name).into_any(),
            ConnectionKey::Entry(index) => items[*index].0.clone(),
        };
        let value: Bound<'py, PyAny> = match source {
            ConnectionSource::Entry(index) => items[*index].1.clone(),
            ConnectionSource::Text(text) => PyString::new(py, text).into_any(),
        };
        connection.set_item(key, value)?;
    }
    new_record_with(
        py,
        RecordId::NormalizedDbtProfileConnection,
        &[
            ("adapter", PyString::new(py, profile.adapter).into_any()),
            ("connection", connection.into_any()),
            (
                "target_schema",
                optional_text(py, profile.target_schema.as_deref()),
            ),
            (
                "target_database",
                optional_text(py, profile.target_database.as_deref()),
            ),
            ("warnings", PyTuple::new(py, &profile.warnings)?.into_any()),
        ],
    )
}

/// Normalize rendered dbt profile output for SQLBuild.
#[pyfunction]
#[pyo3(signature = (*, resolved))]
pub(crate) fn normalize_dbt_profile_connection<'py>(
    py: Python<'py>,
    resolved: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let output: Bound<'py, PyDict> = PyDict::new(py);
    output.update(&native_field(&resolved, "output")?.cast_into()?)?;
    let mut entries: Vec<ProfileEntry> = Vec::new();
    for (key, value) in output.iter() {
        entries.push(ProfileEntry {
            key: if key.is_instance_of::<PyString>() {
                Some(key.extract()?)
            } else {
                None
            },
            value: profile_value(&value)?,
        });
    }
    let profile: NormalizedProfile =
        normalize_dbt_profile(&entries).map_err(|error| raised(py, error))?;
    normalized_connection(&output, &profile)
}

fn text_argument(arguments: &KeywordArguments<'_>, name: &str) -> PyResult<String> {
    arguments.value(name)?.extract()
}

/// The `sqlbuild_project.toml` text `sqb dbt init` writes.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_dbt_init_project_toml(
    py: Python<'_>,
    options: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let arguments: KeywordArguments<'_> = KeywordArguments::read(
        py,
        "build_dbt_init_project_toml",
        options,
        INIT_TOML_OPTIONS,
    )?;
    Ok(dbt_init_project_toml(&DbtInitConfig {
        project_name: text_argument(&arguments, "project_name")?,
        adapter: text_argument(&arguments, "adapter")?,
        target_name: text_argument(&arguments, "target_name")?,
        profile_name: text_argument(&arguments, "profile_name")?,
        dbt_project_dir: text_argument(&arguments, "dbt_project_dir")?,
        profiles_dir: text_argument(&arguments, "profiles_dir")?,
        target_path: text_argument(&arguments, "target_path")?,
        target_schema: arguments.value("target_schema")?.extract()?,
        target_database: arguments.value("target_database")?.extract()?,
    }))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(normalize_dbt_profile_connection, module)?)?;
    module.add_function(wrap_pyfunction!(build_dbt_init_project_toml, module)?)
}
