//! The effective target, its namespace, and the shared project-local compile-cache directory.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, imported};
use crate::bindings::_helpers::compile_attachment::effective_config::project_vars;
use crate::bindings::_helpers::compile_command::constants::{
    COMPILE_CACHE_DISABLE_ENV_VAR, COMPILE_CACHE_DISABLE_VALUE, RETIRED_CACHE_DIRECTORY_NAMES,
};
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_name;
use crate::bindings::_helpers::frontier::cache_directory::cache_directory;
use crate::bindings::_helpers::frontier::constants::{
    COMPILER_CACHE_DIRECTORY_NAME, RETIRED_CACHE_NAMESPACE_SUFFIXES,
};
use crate::bindings::_helpers::model_config::template_data::{early_model_templates, run_id};
use crate::bindings::_helpers::records::fields::field as native_field;
use pyo3::IntoPyObjectExt;

const DATABASE_KEY: &str = "database";
const SCHEMA_KEY: &str = "schema";

/// The selected target's name and merged config, or `(None, None)`.
fn resolved_target<'py>(
    discovered: &Bound<'py, PyAny>,
    selected_target: &Bound<'py, PyAny>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let py: Python<'py> = discovered.py();
    let project_config: Bound<'py, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'py, PyAny> = native_field(discovered, "local_config")?;
    let target_name: Bound<'py, PyAny> = resolve_target_name(
        py,
        &project_config.clone(),
        &local_config.clone(),
        &selected_target.clone(),
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    if target_name.is_none() {
        return Ok((target_name, py.None().into_bound(py)));
    }
    let target_config: Bound<'py, PyAny> =
        resolve_target_config(py, &project_config, &local_config, &target_name.clone())
            .and_then(|value| value.into_bound_py_any(py))?;
    Ok((target_name, target_config))
}

/// Resolve the effective target and shared project-local compile-cache directory.
#[pyfunction]
#[pyo3(signature = (*, discovered_inputs, selected_target, no_cache))]
pub(crate) fn build_compile_target_context<'py>(
    discovered_inputs: &Bound<'py, PyAny>,
    selected_target: &Bound<'py, PyAny>,
    no_cache: bool,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = discovered_inputs.py();
    let (target_name, target_config) = resolved_target(discovered_inputs, selected_target)?;
    let cache_dir: Bound<'py, PyAny> = compile_cache_root(
        &native_field(discovered_inputs, "project_dir")?,
        &target_config,
        no_cache,
    )?;
    PyTuple::new(py, [target_name, target_config, cache_dir])
}

/// The shared compile-cache directory, or `None` when caching is disabled.
fn compile_cache_root<'py>(
    project_dir: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
    no_cache: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let disabled: bool = py
        .import("os")?
        .getattr("environ")?
        .call_method1("get", (COMPILE_CACHE_DISABLE_ENV_VAR,))?
        .extract::<Option<String>>()?
        .is_some_and(|value| value == COMPILE_CACHE_DISABLE_VALUE);
    if no_cache
        || project_dir.is_none()
        || (!target_config.is_none()
            && native_field(target_config, "compile_cache")?
                .is(pyo3::types::PyBool::new(py, false)))
        || disabled
    {
        return Ok(py.None().into_bound(py));
    }
    let root: Bound<'py, PyAny> = cache_directory(project_dir)?;
    remove_retired_caches(&root)?;
    Ok(root)
}

/// Delete the fact and reference caches older releases kept under each retired cache root.
fn remove_retired_caches(root: &Bound<'_, PyAny>) -> PyResult<()> {
    let py: Python<'_> = root.py();
    let rmtree: Bound<'_, PyAny> = imported(py, "shutil", "rmtree")?;
    let ignore: Bound<'_, PyDict> = PyDict::new(py);
    ignore.set_item("ignore_errors", true)?;
    for suffix in RETIRED_CACHE_NAMESPACE_SUFFIXES {
        for name in RETIRED_CACHE_DIRECTORY_NAMES {
            let retired: Bound<'_, PyAny> = native_field(root, "parent")?
                .div(format!("{COMPILER_CACHE_DIRECTORY_NAME}{suffix}"))?
                .div(name)?;
            if retired.call_method0("is_dir")?.is_truthy()? {
                rmtree.call((retired,), Some(&ignore))?;
            }
        }
    }
    Ok(())
}

/// `value if isinstance(value, str) and value else None`.
fn optional_string<'py>(
    value: Option<Bound<'py, PyAny>>,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    Ok(match value {
        Some(value) if value.is_instance_of::<PyString>() && value.is_truthy()? => value,
        _ => py.None().into_bound(py),
    })
}

/// A project-level location: the target's value, then `[defaults]`, then the first set fallback.
pub(crate) fn project_location<'py>(
    (target, default): (Bound<'py, PyAny>, Bound<'py, PyAny>),
    fallbacks: Vec<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = target.py();
    for value in [target, default].into_iter().chain(fallbacks) {
        if value.is_truthy()? {
            return Ok(value);
        }
    }
    Ok(py.None().into_bound(py))
}

/// Whether the top-level `[connection]` is the implicit default target: no named connections or targets.
pub(crate) fn connection_is_default_target(discovered: &Bound<'_, PyAny>) -> PyResult<bool> {
    for config in ["project_config", "local_config"] {
        let config: Bound<'_, PyAny> = native_field(discovered, config)?;
        for field in ["connections", "targets"] {
            if native_field(&config, field)?.is_truthy()? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

/// Resolve the target and its effective database/schema without compiling resources.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn build_effective_target_namespace<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_effective_target_namespace",
        options,
        (
            &[
                "discovered_inputs",
                "selected_target",
                "connection_database",
                "connection_schema",
                "default_database",
                "default_schema",
            ],
            &[],
        ),
    )?;
    let discovered_inputs: &Bound<'py, PyAny> = &arguments.value("discovered_inputs")?;
    let selected_target: &Bound<'py, PyAny> = &arguments.value("selected_target")?;
    let connection_database: Bound<'py, PyAny> = arguments.value("connection_database")?;
    let connection_schema: Bound<'py, PyAny> = arguments.value("connection_schema")?;
    let default_database: Bound<'py, PyAny> = arguments.value("default_database")?;
    let default_schema: Bound<'py, PyAny> = arguments.value("default_schema")?;
    let (target_name, target_config) = resolved_target(discovered_inputs, selected_target)?;
    let effective_vars: Bound<'py, PyAny> = project_vars(discovered_inputs, &target_config)?;
    let target_value = |key: &str| -> PyResult<Bound<'py, PyAny>> {
        if target_config.is_none() {
            return Ok(py.None().into_bound(py));
        }
        native_field(&target_config, key)
    };
    let project_config: Bound<'py, PyAny> = native_field(discovered_inputs, "project_config")?;
    let defaults: Bound<'py, PyAny> = native_field(&project_config, "defaults")?;
    let inline: bool = connection_is_default_target(discovered_inputs)?;
    let values: Bound<'py, PyDict> = PyDict::new(py);
    values.set_item(
        DATABASE_KEY,
        project_location(
            (
                target_value(DATABASE_KEY)?,
                native_field(&defaults, DATABASE_KEY)?,
            ),
            [connection_database]
                .into_iter()
                .filter(|_| inline)
                .chain([default_database])
                .collect(),
        )?,
    )?;
    values.set_item(
        SCHEMA_KEY,
        project_location(
            (
                target_value(SCHEMA_KEY)?,
                native_field(&defaults, SCHEMA_KEY)?,
            ),
            [connection_schema]
                .into_iter()
                .filter(|_| inline)
                .chain([default_schema])
                .collect(),
        )?,
    )?;
    let run_id: Bound<'py, PyAny> = run_id(&py.None().into_bound(py))?;
    let resolved: Bound<'py, PyDict> =
        early_model_templates(values.as_any(), &effective_vars, (&target_name, &run_id))?
            .downcast_into()?;
    PyTuple::new(
        py,
        [
            target_name,
            target_config,
            optional_string(resolved.get_item(DATABASE_KEY)?, py)?,
            optional_string(resolved.get_item(SCHEMA_KEY)?, py)?,
        ],
    )
}
