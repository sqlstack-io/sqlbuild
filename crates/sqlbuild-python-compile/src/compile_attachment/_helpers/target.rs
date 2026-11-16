//! The effective target, its namespace, and the shared project-local compile-cache directory.

use crate::compile_attachment::main::connection_is_default_target::connection_is_default_target;
use crate::compile_attachment::main::project_location::project_location;
use crate::compile_attachment::main::resolved_target::resolved_target;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};

use crate::compile_command::constants::COMPILE_CACHE_DISABLE_ENV_VAR;
use crate::compile_command::constants::COMPILE_CACHE_DISABLE_VALUE;
use crate::compile_command::constants::RETIRED_CACHE_DIRECTORY_NAMES;
use crate::frontier::constants::COMPILER_CACHE_DIRECTORY_NAME;
use crate::frontier::constants::RETIRED_CACHE_NAMESPACE_SUFFIXES;
use crate::frontier::main::cache_directory::cache_directory;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_front::compile_attachment::main::project_vars::project_vars;
use sqlbuild_python_front::model_config::main::config::early_model_templates::early_model_templates;
use sqlbuild_python_front::model_config::main::config::run_id::run_id;

const DATABASE_KEY: &str = "database";
const SCHEMA_KEY: &str = "schema";
/// Adapters whose database is a file, so a connection `database` is not a target location.
pub(crate) const NAMESPACELESS_ADAPTERS: [&str; 2] = ["duckdb", "motherduck"];

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

/// The selected target's name, record, database and schema, resolved from `connection` and the adapter's defaults without compiling resources.
pub(crate) fn effective_target_namespace<'py>(
    (discovered_inputs, selected_target): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (adapter_name, connection, adapter): (&str, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = adapter.py();
    let connection_database: Bound<'py, PyAny> = if NAMESPACELESS_ADAPTERS.contains(&adapter_name) {
        py.None().into_bound(py)
    } else {
        mapping_get(connection, DATABASE_KEY)?
    };
    let connection_schema: Bound<'py, PyAny> = mapping_get(connection, SCHEMA_KEY)?;
    let default_database: Bound<'py, PyAny> = adapter.call_method0("default_database")?;
    let default_schema: Bound<'py, PyAny> = adapter.call_method0("default_schema")?;
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
