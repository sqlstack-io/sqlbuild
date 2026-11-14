//! Adapter-aware target namespace validation of a compile.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyString, PyStringMethods};
use pyo3::{pyfunction, wrap_pyfunction};

use sqlbuild_analysis::assembly::target_checks::constants::{
    LOADER_COLLISION_CODE, MANAGED_WRITE_SCHEMA_CODE, MISSING_TARGET_PARTS_CODE,
    NAMED_TARGET_SCHEMA_CODE,
};
use sqlbuild_analysis::assembly::target_checks::errors::TargetError;
use sqlbuild_analysis::assembly::target_checks::main::explicit_write_schema::explicit_write_schema;
use sqlbuild_analysis::assembly::target_checks::main::first_shared_namespace::first_shared_namespace;
use sqlbuild_analysis::assembly::target_checks::main::missing_target_parts::missing_target_parts;
use sqlbuild_analysis::assembly::target_checks::main::target_error_message::target_error_message;
use sqlbuild_analysis::assembly::target_checks::models::{LoaderCollision, MissingParts};
use sqlbuild_core::text::main::python_strip::python_strip;

use sqlbuild_model_config::templates::models::TemplateOptions;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, keywords};
use crate::bindings::_helpers::model_config::template_data::expanded_template_data;

/// Connection settings that identify the warehouse two targets write to.
const CONNECTION_IDENTITY_KEYS: [&str; 7] = [
    "account",
    "host",
    "port",
    "project",
    "server",
    "server_hostname",
    "workspace_url",
];
/// Adapters whose execution needs every model and seed database and schema resolved.
const EXPLICIT_TARGET_ADAPTERS: [&str; 3] = ["snowflake", "bigquery", "databricks"];
const LOADER_NAMESPACE_LABEL: &str = "managed loader target namespace";
const DEFAULTS_HELP_PURPOSE: &str = "to set project-wide defaults";
const NAMED_TARGET_HELP_PURPOSE: &str = "to build this target into that schema";
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_connection_config;
use crate::bindings::_helpers::compile_attachment::effective_config::build_effective_runtime_config;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_config;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_target_name;
use crate::bindings::_helpers::discovery::guidance::setting_help;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::mapping_get;
use crate::bindings::_helpers::records::fields::mapping_items;
use crate::bindings::_helpers::records::ids::ErrorId;
use pyo3::IntoPyObjectExt;
use sqlbuild_core::constants::PROJECT_CONFIG_FILENAME;
const DEFAULTS_SECTION: &str = "defaults";

/// One optional `(database, schema)` namespace.
type Namespace = (Option<String>, Option<String>);
/// A target's sorted connection identity and the namespaces its managed sources write.
type TargetNamespaces = (Vec<(String, String)>, Vec<Namespace>);

/// A `PlannerInputError` with `message`, `code` and optional `help`.
fn planner_error(py: Python<'_>, message: String, code: &str, help: Option<String>) -> PyErr {
    let raised: PyResult<Bound<'_, PyAny>> = (|| {
        error_class(py, ErrorId::PlannerInput)?.call(
            (message,),
            Some(&keywords(
                py,
                &[
                    ("code", PyString::new(py, code).into_any()),
                    (
                        "help",
                        help.map_or_else(
                            || py.None().into_bound(py),
                            |help| PyString::new(py, &help).into_any(),
                        ),
                    ),
                ],
            )?),
        )
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// `value` as an optional string: `None`, or a `str`'s text.
fn optional_text(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        return Ok(None);
    }
    Ok(Some(value.extract()?))
}

/// `value` when it is a `str`, else `None`.
fn text_or_none(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    match value.downcast::<PyString>() {
        Ok(text) => Ok(Some(text.to_cow()?.into_owned())),
        Err(_) => Ok(None),
    }
}

/// The connection's `part` when the compile leaves it unused because the project names connections or targets.
fn ignored_connection_value(project: &Bound<'_, PyAny>, part: &str) -> PyResult<Option<String>> {
    if text_or_none(&native_field(project, &format!("effective_target_{part}"))?)?.is_some() {
        return Ok(None);
    }
    text_or_none(&mapping_get(
        &native_field(project, "effective_connection")?,
        part,
    )?)
}

/// The sentence saying the connection's `part` is unused.
fn ignored_connection_note(part: &str, value: &str) -> String {
    format!(
        " The connection's {part} '{value}' is not used because the project defines named \
         connections or targets."
    )
}

/// `setting_help` for adding `key = "value"` under `[section]` of the project file.
fn project_setting_help(
    py: Python<'_>,
    (purpose, section): (&str, &str),
    (key, value): (&str, &str),
) -> PyResult<String> {
    setting_help(
        py,
        Some(keywords(
            py,
            &[
                ("purpose", PyString::new(py, purpose).into_any()),
                (
                    "file_name",
                    PyString::new(py, PROJECT_CONFIG_FILENAME).into_any(),
                ),
                ("section", PyString::new(py, section).into_any()),
                ("key", PyString::new(py, key).into_any()),
                ("value", PyString::new(py, value).into_any()),
            ],
        )?),
    )
    .and_then(|value| value.into_bound_py_any(py))?
    .extract()
}

/// Require shared-warehouse named targets to choose a schema strategy.
#[pyfunction]
#[pyo3(signature = (**options))]
fn validate_named_target_schema_strategy<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> Result<(), PyErr> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "validate_named_target_schema_strategy",
        options,
        (&["discovered_inputs", "adapter", "selected_target"], &[]),
    )?;
    named_target_schema_strategy(
        &arguments.value("discovered_inputs")?,
        &arguments.value("adapter")?,
        &arguments.value("selected_target")?,
    )
}

/// `validate_named_target_schema_strategy` with its arguments.
pub(crate) fn named_target_schema_strategy(
    discovered: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
    selected_target: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = discovered.py();
    if native_field(adapter, "allows_implicit_managed_write_schema")?.is_truthy()? {
        return Ok(());
    }
    let project_config: Bound<'_, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'_, PyAny> = native_field(discovered, "local_config")?;
    let target_name: Bound<'_, PyAny> = resolve_target_name(
        py,
        &project_config.clone(),
        &local_config.clone(),
        &selected_target.clone(),
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    if target_name.is_none() {
        return Ok(());
    }
    let target_config: Bound<'_, PyAny> =
        resolve_target_config(py, &project_config, &local_config, &target_name.clone())
            .and_then(|value| value.into_bound_py_any(py))?;
    let schema: Option<String> = text_or_none(&native_field(&target_config, "schema")?)?;
    if schema.is_some_and(|schema| !python_strip(&schema).is_empty()) {
        return Ok(());
    }
    let name: String = target_name.str()?.to_cow()?.into_owned();
    let mut message: String = target_error_message(&TargetError::NamedTargetSchema(name.clone()));
    let connection: Bound<'_, PyAny> =
        build_effective_connection_config(discovered, Some(target_name), None, false)?;
    let help: Option<String> = match text_or_none(&mapping_get(&connection, "schema")?)? {
        Some(value) => {
            message.push_str(&ignored_connection_note("schema", &value));
            Some(project_setting_help(
                py,
                (NAMED_TARGET_HELP_PURPOSE, &format!("targets.{name}")),
                ("schema", &value),
            )?)
        }
        None => None,
    };
    Err(planner_error(py, message, NAMED_TARGET_SCHEMA_CODE, help))
}

/// Reject managed loader write namespaces shared by multiple targets.
#[pyfunction]
#[pyo3(signature = (**options))]
fn validate_managed_loader_target_isolation<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> Result<(), PyErr> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "validate_managed_loader_target_isolation",
        options,
        (&["discovered_inputs", "adapter"], &["resolved_connection"]),
    )?;
    managed_loader_target_isolation(
        &arguments.value("discovered_inputs")?,
        &arguments.value("adapter")?,
        &arguments.value("resolved_connection")?,
    )
}

/// `validate_managed_loader_target_isolation` with its arguments.
pub(crate) fn managed_loader_target_isolation(
    discovered: &Bound<'_, PyAny>,
    adapter: &Bound<'_, PyAny>,
    resolved_connection: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = discovered.py();
    let mut managed_sources: Vec<Bound<'_, PyAny>> = Vec::new();
    for source_file in native_field(discovered, "source_files")?.try_iter()? {
        for entry in native_field(&source_file?, "source_entries")?.try_iter()? {
            let entry: Bound<'_, PyAny> = entry?;
            if native_field(&entry, "managed")?.is_truthy()? {
                managed_sources.push(entry);
            }
        }
    }
    if managed_sources.is_empty() {
        return Ok(());
    }
    let project_config: Bound<'_, PyAny> = native_field(discovered, "project_config")?;
    let local_config: Bound<'_, PyAny> = native_field(discovered, "local_config")?;
    let mut target_names: Vec<String> = Vec::new();
    for config in [&project_config, &local_config] {
        for name in native_field(config, "targets")?.try_iter()? {
            let name: String = name?.extract()?;
            if !target_names.contains(&name) {
                target_names.push(name);
            }
        }
    }
    target_names.sort();
    let mut targets: Vec<TargetNamespaces> = Vec::new();
    for target_name in &target_names {
        targets.push(target_namespaces(
            (discovered, adapter, resolved_connection),
            target_name,
            &managed_sources,
        )?);
    }
    for (index, left_target) in target_names.iter().enumerate() {
        for (offset, right_target) in target_names[index + 1..].iter().enumerate() {
            let (left, right) = (&targets[index], &targets[index + 1 + offset]);
            if left.0 != right.0 {
                continue;
            }
            let shared: Vec<Namespace> = left
                .1
                .iter()
                .filter(|namespace| right.1.contains(namespace))
                .cloned()
                .collect();
            let Some((database, schema)) = first_shared_namespace(&shared) else {
                continue;
            };
            return Err(planner_error(
                py,
                target_error_message(&TargetError::LoaderCollision(LoaderCollision {
                    left_target: left_target.clone(),
                    right_target: right_target.clone(),
                    database,
                    schema,
                })),
                LOADER_COLLISION_CODE,
                None,
            ));
        }
    }
    Ok(())
}

/// One target's connection identity and the namespaces its managed sources write.
fn target_namespaces(
    (discovered, adapter, resolved_connection): (
        &Bound<'_, PyAny>,
        &Bound<'_, PyAny>,
        &Bound<'_, PyAny>,
    ),
    target_name: &str,
    managed_sources: &[Bound<'_, PyAny>],
) -> PyResult<TargetNamespaces> {
    let py: Python<'_> = discovered.py();
    let name: Bound<'_, PyAny> = PyString::new(py, target_name).into_any();
    let target_config: Bound<'_, PyAny> = resolve_target_config(
        py,
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
        &name.clone(),
    )
    .and_then(|value| value.into_bound_py_any(py))?;
    let runtime: Bound<'_, PyAny> =
        build_effective_runtime_config(&discovered.clone(), Some(name.clone()), None)
            .and_then(|value| value.into_bound_py_any(py))?;
    let (_, effective_vars, _): (Bound<'_, PyAny>, Bound<'_, PyAny>, Bound<'_, PyAny>) =
        runtime.extract()?;
    let connection: Bound<'_, PyAny> = if resolved_connection.is_none() {
        build_effective_connection_config(&discovered.clone(), Some(name), None, false)
            .and_then(|value| value.into_bound_py_any(py))?
    } else {
        resolved_connection.clone()
    };
    let mut identity: Vec<(String, String)> = Vec::new();
    for item in mapping_items(&connection)?.try_iter()? {
        let item: Bound<'_, PyAny> = item?;
        let key: Bound<'_, PyAny> = item.get_item(0)?;
        let Some(key) = text_or_none(&key)? else {
            continue;
        };
        if CONNECTION_IDENTITY_KEYS.contains(&key.as_str()) {
            identity.push((key, item.get_item(1)?.repr()?.to_cow()?.into_owned()));
        }
    }
    identity.sort();
    let mut namespaces: Vec<Namespace> = Vec::new();
    for source_entry in managed_sources {
        let namespace: Namespace = managed_source_namespace(
            source_entry,
            (&target_config, &connection, &effective_vars),
            adapter,
        )?;
        if !namespaces.contains(&namespace) {
            namespaces.push(namespace);
        }
    }
    Ok((identity, namespaces))
}

/// The first truthy value of `candidates`, else `fallback()`.
fn first_truthy<'py>(
    candidates: &[Bound<'py, PyAny>],
    fallback: impl FnOnce() -> PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    for candidate in candidates {
        if candidate.is_truthy()? {
            return Ok(candidate.clone());
        }
    }
    fallback()
}

/// `_managed_source_namespace`: the database and schema a managed source loads into.
fn managed_source_namespace(
    source_entry: &Bound<'_, PyAny>,
    (target_config, connection, effective_vars): (
        &Bound<'_, PyAny>,
        &Bound<'_, PyAny>,
        &Bound<'_, PyAny>,
    ),
    adapter: &Bound<'_, PyAny>,
) -> PyResult<Namespace> {
    let py: Python<'_> = source_entry.py();
    let database: Bound<'_, PyAny> = first_truthy(
        &[
            native_field(source_entry, "database")?,
            native_field(target_config, "database")?,
            mapping_get(connection, "database")?,
        ],
        || adapter.call_method0("default_database"),
    )?;
    let schema: Bound<'_, PyAny> = first_truthy(
        &[
            native_field(source_entry, "schema")?,
            native_field(target_config, "loader_schema")?,
            native_field(target_config, "schema")?,
            mapping_get(connection, "schema")?,
        ],
        || adapter.call_method0("default_schema"),
    )?;
    let value: Bound<'_, PyDict> = PyDict::new(py);
    pyo3::types::PyDictMethods::set_item(&value, "database", database)?;
    pyo3::types::PyDictMethods::set_item(&value, "schema", schema)?;
    let expanded: Bound<'_, PyAny> = expanded_template_data(
        value.as_any(),
        (effective_vars.downcast()?, &PyDict::new(py)),
        (
            TemplateOptions {
                allow_context: false,
                preserve_context_tokens: false,
                preserve_unknown_context: false,
            },
            LOADER_NAMESPACE_LABEL,
        ),
    )?;
    let part = |key: &str| -> PyResult<Option<String>> {
        let value: Bound<'_, PyAny> = expanded.get_item(key)?;
        if value.is_none() {
            return Ok(None);
        }
        Ok(Some(value.str()?.to_cow()?.into_owned()))
    };
    Ok((part("database")?, part("schema")?))
}

/// Validate compiled model and seed locations for the effective adapter.
#[pyfunction]
#[pyo3(signature = (**options))]
fn validate_project_targets<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> Result<(), PyErr> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "validate_project_targets",
        options,
        (&["adapter_name", "project"], &[]),
    )?;
    project_targets(
        &arguments.value("adapter_name")?,
        &arguments.value("project")?,
    )
}

/// Whether the adapter needs every model and seed database and schema resolved.
pub(crate) fn requires_explicit_targets(adapter_name: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(text_or_none(adapter_name)?
        .as_deref()
        .is_some_and(|name| EXPLICIT_TARGET_ADAPTERS.contains(&name)))
}

/// `validate_project_targets` with its arguments.
pub(crate) fn project_targets(
    adapter_name: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<()> {
    if !requires_explicit_targets(adapter_name)? {
        return Ok(());
    }
    let adapter_label: String = adapter_name.str()?.to_cow()?.into_owned();
    for (resource_kind, attribute) in [("model", "models"), ("seed", "seeds")] {
        let mut targets: Vec<(String, Bound<'_, PyAny>)> = Vec::new();
        let mut positions: HashMap<String, usize> = HashMap::new();
        for resource in native_field(project, attribute)?.try_iter()? {
            let resource: Bound<'_, PyAny> = resource?;
            let resource_name: String = native_field(&resource, "name")?
                .str()?
                .to_cow()?
                .into_owned();
            let destination: Bound<'_, PyAny> = native_field(&resource, "destination")?;
            match positions.get(&resource_name) {
                Some(position) => targets[*position].1 = destination,
                None => {
                    positions.insert(resource_name.clone(), targets.len());
                    targets.push((resource_name, destination));
                }
            }
        }
        for (resource_name, target) in targets {
            required_target_parts(
                (&adapter_label, resource_kind),
                resource_name,
                &target,
                project,
            )?;
        }
    }
    Ok(())
}

/// Raise when `target` misses its database or schema, naming any connection value left unused.
fn required_target_parts(
    (adapter_name, resource_kind): (&str, &str),
    resource_name: String,
    target: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let py: Python<'_> = target.py();
    let database: Option<String> = optional_text(&native_field(target, "database")?)?;
    let schema: Option<String> = optional_text(&native_field(target, "schema")?)?;
    let missing: Vec<&str> = missing_target_parts(database.as_deref(), schema.as_deref());
    let Some(first) = missing.first() else {
        return Ok(());
    };
    let mut message: String = target_error_message(&TargetError::MissingParts(MissingParts {
        adapter_name: adapter_name.to_owned(),
        resource_kind: resource_kind.to_owned(),
        resource_name,
        database,
        schema,
    }));
    let mut first_value: String = format!("my_{first}");
    for part in &missing {
        if let Some(value) = ignored_connection_value(project, part)? {
            message.push_str(&ignored_connection_note(part, &value));
            if part == first {
                first_value = value;
            }
        }
    }
    let help: String = project_setting_help(
        py,
        (DEFAULTS_HELP_PURPOSE, DEFAULTS_SECTION),
        (first, &first_value),
    )?;
    Err(planner_error(
        py,
        message,
        MISSING_TARGET_PARTS_CODE,
        Some(help),
    ))
}

/// Require an explicitly resolved physical schema for every managed warehouse write.
#[pyfunction]
#[pyo3(signature = (**options))]
fn validate_managed_write_schemas<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> Result<(), PyErr> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "validate_managed_write_schemas",
        options,
        (&["adapter", "project"], &[]),
    )?;
    managed_write_schemas(&arguments.value("adapter")?, &arguments.value("project")?)
}

/// `validate_managed_write_schemas` with its arguments.
pub(crate) fn managed_write_schemas(
    adapter: &Bound<'_, PyAny>,
    project: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let ignored: Option<String> = ignored_connection_value(project, "schema")?;
    if ignored.is_none()
        && native_field(adapter, "allows_implicit_managed_write_schema")?.is_truthy()?
    {
        return Ok(());
    }
    let project_schema: Option<String> = explicit_write_schema(
        text_or_none(&native_field(project, "effective_target_schema")?)?.as_deref(),
    );
    for (label, attribute) in [
        ("Model", "models"),
        ("Seed", "seeds"),
        ("Function", "functions"),
    ] {
        for resource in native_field(project, attribute)?.try_iter()? {
            let resource: Bound<'_, PyAny> = resource?;
            managed_write_schema(
                &format!(
                    "{label} '{}'",
                    native_field(&resource, "name")?.str()?.to_cow()?
                ),
                &native_field(&resource, "destination")?.getattr("schema")?,
                (project_schema.as_deref(), ignored.as_deref()),
            )?;
        }
    }
    for source in native_field(project, "sources")?.try_iter()? {
        let source: Bound<'_, PyAny> = source?;
        let entry: Bound<'_, PyAny> = native_field(&source, "source_entry")?;
        if !native_field(&entry, "managed")?.is_truthy()? {
            continue;
        }
        managed_write_schema(
            &format!(
                "Managed source '{}'",
                native_field(&source, "name")?.str()?.to_cow()?
            ),
            &native_field(&entry, "schema")?,
            (project_schema.as_deref(), ignored.as_deref()),
        )?;
    }
    if ignored.is_none() {
        return Ok(());
    }
    let mut source_loaders: HashSet<String> = HashSet::new();
    for source in native_field(project, "sources")?.try_iter()? {
        if let Some(loader) = text_or_none(&native_field(
            &native_field(&source?, "source_entry")?,
            "loader",
        )?)? {
            source_loaders.insert(loader);
        }
    }
    for loader in native_field(project, "loader_functions")?.try_iter()? {
        let loader: Bound<'_, PyAny> = loader?;
        let destination: Option<String> = text_or_none(&native_field(&loader, "destination")?)?;
        if destination.is_some_and(|destination| destination.contains('.'))
            || source_loaders.contains(&native_field(&loader, "name")?.extract::<String>()?)
        {
            continue;
        }
        managed_write_schema(
            &format!(
                "Intermediate loader '{}'",
                native_field(&loader, "name")?.str()?.to_cow()?
            ),
            &loader.py().None().into_bound(loader.py()),
            (project_schema.as_deref(), ignored.as_deref()),
        )?;
    }
    Ok(())
}

/// Raise when neither the resource nor the project names a physical write schema.
pub(crate) fn managed_write_schema(
    resource_label: &str,
    schema: &Bound<'_, PyAny>,
    (project_schema, ignored): (Option<&str>, Option<&str>),
) -> PyResult<()> {
    if explicit_write_schema(text_or_none(schema)?.as_deref()).is_some() || project_schema.is_some()
    {
        return Ok(());
    }
    let py: Python<'_> = schema.py();
    let mut message: String =
        target_error_message(&TargetError::ManagedWriteSchema(resource_label.to_owned()));
    let help: Option<String> = match ignored {
        Some(value) => {
            message.push_str(&ignored_connection_note("schema", value));
            Some(project_setting_help(
                py,
                (DEFAULTS_HELP_PURPOSE, DEFAULTS_SECTION),
                ("schema", value),
            )?)
        }
        None => None,
    };
    Err(planner_error(py, message, MANAGED_WRITE_SCHEMA_CODE, help))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        validate_named_target_schema_strategy,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        validate_managed_loader_target_isolation,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(validate_project_targets, module)?)?;
    module.add_function(wrap_pyfunction!(validate_managed_write_schemas, module)?)
}
