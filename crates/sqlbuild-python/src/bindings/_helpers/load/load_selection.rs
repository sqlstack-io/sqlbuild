//! Resolve which loaders and sources `sqb load` runs, as `SourceEntry` objects.

use std::collections::{HashMap, HashSet};

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::load_command::errors::UnknownLoadSelector;
use sqlbuild_core::load_command::main::load_reference_loaders::load_reference_loaders;
use sqlbuild_core::load_command::main::select_load_entries::select_load_entries as select_entries;
use sqlbuild_core::load_command::models::{LoadEntry, LoadGraph};
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compile_attachment::effective_config::{
    build_effective_connection_config, build_effective_runtime_config,
};
use crate::bindings::_helpers::compile_attachment::target::{
    connection_is_default_target, project_location,
};
use crate::bindings::_helpers::compile_driver::targets::{
    managed_write_schema, preserved_loader_schemas,
};
use crate::bindings::_helpers::contracts::spec_resolution::loader_destination_parts;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::model_config::template_data::early_model_templates;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced_keywords;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

const LOADER_NODE_KEY: &str = "sqlbuild_loader_node";
const LOADER_TABLE_PREFIX: &str = "__loader__";
const UNKNOWN_SELECTOR_CODE: &str = "C901";
const UNKNOWN_SELECTOR_HELP: &str = "Use exact managed source names or loader function names.";

/// The default database and schema of intermediate loader tables.
pub(crate) struct LoaderDefaults<'py> {
    pub(crate) database: Bound<'py, PyAny>,
    pub(crate) schema: Bound<'py, PyAny>,
}

/// Discovered loader functions by name and the selection graph over managed sources.
pub(crate) struct LoadProject<'py> {
    pub(crate) sources: Vec<Bound<'py, PyAny>>,
    loaders: HashMap<String, Bound<'py, PyAny>>,
    graph: LoadGraph,
}

/// Every declared source with the target's database and schema filled in where unset.
pub(crate) fn environment_sources<'py>(
    discovered: &Bound<'py, PyAny>,
    target_config: &Bound<'py, PyAny>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let py: Python<'py> = discovered.py();
    let mut sources: Vec<Bound<'py, PyAny>> = Vec::new();
    for source_file in native_field(discovered, "source_files")?.try_iter()? {
        for source in native_field(&source_file?, "source_entries")?.try_iter()? {
            let source: Bound<'py, PyAny> = source?;
            let changes: Bound<'py, PyDict> = PyDict::new(py);
            for field in ["database", "schema"] {
                let value: Bound<'py, PyAny> = native_field(&source, field)?;
                let effective: Bound<'py, PyAny> = if !value.is_none() || target_config.is_none() {
                    value
                } else {
                    native_field(target_config, field)?
                };
                changes.set_item(field, effective)?;
            }
            sources.push(replaced_keywords(&source, &changes)?);
        }
    }
    Ok(sources)
}

impl<'py> LoadProject<'py> {
    pub(crate) fn new(
        discovered: &Bound<'py, PyAny>,
        target_config: &Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        let py: Python<'py> = discovered.py();
        let sources: Vec<Bound<'py, PyAny>> = environment_sources(discovered, target_config)?;
        let mut graph: LoadGraph = LoadGraph::default();
        for source in &sources {
            let loader: Bound<'py, PyAny> = native_field(source, "loader")?;
            if !loader.is_none() {
                graph
                    .sources
                    .push((native_field(source, "name")?.extract()?, loader.extract()?));
            }
        }
        let functions: Vec<Bound<'py, PyAny>> = native_field(discovered, "loader_functions")?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        let names_by_function: Bound<'py, PyDict> = PyDict::new(py);
        for function in &functions {
            names_by_function.set_item(
                native_field(function, "function")?,
                native_field(function, "name")?,
            )?;
        }
        let mut loaders: HashMap<String, Bound<'py, PyAny>> = HashMap::new();
        for function in functions {
            let name: String = native_field(&function, "name")?.extract()?;
            let mut dependencies: Vec<String> = Vec::new();
            for dependency in native_field(&function, "depends_on")?.try_iter()? {
                if let Some(dependency) = names_by_function.get_item(dependency?)? {
                    dependencies.push(dependency.extract()?);
                }
            }
            graph.loaders.push((name.clone(), dependencies));
            loaders.insert(name, function);
        }
        Ok(Self {
            sources,
            loaders,
            graph,
        })
    }

    /// The `SourceEntry` an intermediate loader writes, under the loader defaults.
    fn loader_entry(
        &self,
        name: &str,
        defaults: &LoaderDefaults<'py>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let loader: &Bound<'py, PyAny> = &self.loaders[name];
        let py: Python<'py> = loader.py();
        let destination: Bound<'py, PyAny> = native_field(loader, "destination")?;
        let destination: Bound<'py, PyAny> = if destination.is_none() {
            PyString::new(py, &format!("{LOADER_TABLE_PREFIX}{name}")).into_any()
        } else {
            destination
        };
        let parts: Bound<'py, PyAny> = loader_destination_parts(
            py,
            &destination.extract::<String>()?,
            Some(defaults.database.clone()).filter(|value| !value.is_none()),
            Some(defaults.schema.clone()).filter(|value| !value.is_none()),
        )?;
        let meta: Bound<'py, PyDict> = PyDict::new(py);
        meta.set_item(LOADER_NODE_KEY, true)?;
        let name_value: Bound<'py, PyAny> = PyString::new(py, name).into_any();
        new_record_with(
            py,
            RecordId::SourceEntry,
            &[
                ("name", name_value.clone()),
                ("database", native_field(&parts, "database")?),
                ("schema", native_field(&parts, "schema")?),
                ("table", native_field(&parts, "table")?),
                ("loader", name_value),
                ("write_strategy", native_field(loader, "write_strategy")?),
                ("cursor_column", native_field(loader, "cursor_column")?),
                ("unique_key", native_field(loader, "unique_key")?),
                ("contract", native_field(loader, "contract")?),
                ("meta", meta.into_any()),
                ("columns", native_field(loader, "columns")?),
            ],
        )
    }

    /// The selected loader and source entries in execution order.
    pub(crate) fn selected_entries(
        &self,
        (select, exclude): (&[String], &[String]),
        defaults: &LoaderDefaults<'py>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let py: Python<'py> = defaults.database.py();
        let entries: Vec<LoadEntry> = select_entries(&self.graph, (select, exclude))
            .map_err(|error| unknown_selector(py, &error))?;
        let mut managed: HashMap<String, &Bound<'py, PyAny>> = HashMap::new();
        for source in &self.sources {
            if !native_field(source, "loader")?.is_none() {
                managed.insert(native_field(source, "name")?.extract()?, source);
            }
        }
        entries
            .iter()
            .map(|entry| match entry {
                LoadEntry::Loader(name) => self.loader_entry(name, defaults),
                LoadEntry::Source(name) => Ok(managed[name].clone()),
            })
            .collect()
    }

    /// Unselected upstream loaders the selected entries only reference.
    pub(crate) fn reference_entries(
        &self,
        selected: &[Bound<'py, PyAny>],
        defaults: &LoaderDefaults<'py>,
    ) -> PyResult<Vec<Bound<'py, PyAny>>> {
        let pairs: Vec<(String, Option<String>)> = selected
            .iter()
            .map(|source| {
                Ok((
                    native_field(source, "name")?.extract()?,
                    native_field(source, "loader")?.extract()?,
                ))
            })
            .collect::<PyResult<_>>()?;
        load_reference_loaders(&self.graph, &pairs)
            .iter()
            .map(|name| self.loader_entry(name, defaults))
            .collect()
    }
}

fn unknown_selector(py: Python<'_>, error: &UnknownLoadSelector) -> PyErr {
    let raised = (|| {
        let options: Bound<'_, PyDict> = PyDict::new(py);
        options.set_item("code", UNKNOWN_SELECTOR_CODE)?;
        options.set_item("help", UNKNOWN_SELECTOR_HELP)?;
        error_class(py, ErrorId::CliUser)?.call(
            (format!(
                "sqb load selector '{}' does not match any managed source or loader",
                error.name
            ),),
            Some(&options),
        )
    })();
    match raised {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// `value.strip()` when it is a string with non-blank text, else `None`.
fn non_empty_text<'py>(value: Option<Bound<'py, PyAny>>) -> PyResult<Option<Bound<'py, PyAny>>> {
    let Some(value) = value else { return Ok(None) };
    if !value.is_instance_of::<PyString>() {
        return Ok(None);
    }
    let stripped: Bound<'py, PyAny> = value.call_method0("strip")?;
    Ok(stripped.is_truthy()?.then_some(stripped))
}

/// The target's value for `field`, or `None` when there is no target.
fn target_value<'py>(
    target_config: &Bound<'py, PyAny>,
    field: &str,
) -> PyResult<Bound<'py, PyAny>> {
    if target_config.is_none() {
        return Ok(target_config.clone());
    }
    native_field(target_config, field)
}

/// Intermediate loader defaults: the project location models use (see `project_location`).
pub(crate) fn loader_defaults<'py>(
    discovered: &Bound<'py, PyAny>,
    (selected_target, target_config, cli_vars): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
) -> PyResult<LoaderDefaults<'py>> {
    let py: Python<'py> = discovered.py();
    let connection: Bound<'py, PyDict> = build_effective_connection_config(
        &discovered.clone(),
        Some(selected_target.clone()).filter(|value| !value.is_none()),
        Some(cli_vars.clone()).filter(|value| !value.is_none()),
        false,
    )?
    .downcast_into()?;
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(discovered, "project_config")?,
        &native_field(discovered, "local_config")?,
    )?;
    let duckdb: Bound<'py, PyAny> =
        host::entry(py, AdapterHost::BuiltinAdapter)?.getattr("DUCKDB")?;
    let connection_database: Option<Bound<'py, PyAny>> = if adapter_name.eq(duckdb)? {
        None
    } else {
        non_empty_text(connection.get_item("database")?)?
    };
    let inline: bool = connection_is_default_target(discovered)?;
    let defaults: Bound<'py, PyAny> =
        native_field(&native_field(discovered, "project_config")?, "defaults")?;
    let location = |field: &str, connection_value: Option<Bound<'py, PyAny>>| {
        project_location(
            (
                target_value(target_config, field)?,
                native_field(&defaults, field)?,
            ),
            connection_value.into_iter().filter(|_| inline).collect(),
        )
    };
    let values: Bound<'py, PyDict> = PyDict::new(py);
    values.set_item("database", location("database", connection_database)?)?;
    values.set_item(
        "schema",
        location("schema", non_empty_text(connection.get_item("schema")?)?)?,
    )?;
    let runtime: Bound<'py, PyTuple> = build_effective_runtime_config(
        discovered,
        Some(selected_target.clone()).filter(|value| !value.is_none()),
        Some(cli_vars.clone()).filter(|value| !value.is_none()),
    )?;
    let resolved: Bound<'py, PyAny> = early_model_templates(
        values.as_any(),
        &runtime.get_item(1)?,
        (&runtime.get_item(0)?, &runtime.get_item(2)?),
    )?;
    Ok(LoaderDefaults {
        database: resolved.get_item("database")?,
        schema: resolved.get_item("schema")?,
    })
}

/// Raise when a selected loader would write to `preserve` or lost its schema to an ignored connection.
pub(crate) fn loader_write_schemas<'py>(
    discovered: &Bound<'py, PyAny>,
    (selected_target, cli_vars): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    entries: &[Bound<'py, PyAny>],
) -> PyResult<()> {
    let mut intermediate_loaders: HashSet<String> = HashSet::new();
    for entry in entries {
        let meta: Bound<'py, PyAny> = native_field(entry, "meta")?;
        let flag: Option<Bound<'py, PyAny>> = match meta.downcast::<PyDict>() {
            Ok(meta) => meta.get_item(LOADER_NODE_KEY)?,
            Err(_) => None,
        };
        if flag
            .map(|flag| flag.is_truthy())
            .transpose()?
            .unwrap_or(false)
        {
            intermediate_loaders.insert(native_field(entry, "name")?.extract()?);
        }
    }
    preserved_loader_schemas(discovered, selected_target, &intermediate_loaders)?;
    if connection_is_default_target(discovered)? {
        return Ok(());
    }
    let connection: Bound<'py, PyDict> = build_effective_connection_config(
        &discovered.clone(),
        Some(selected_target.clone()).filter(|value| !value.is_none()),
        Some(cli_vars.clone()).filter(|value| !value.is_none()),
        false,
    )?
    .downcast_into()?;
    let Some(ignored) = non_empty_text(connection.get_item("schema")?)? else {
        return Ok(());
    };
    let ignored: String = ignored.extract()?;
    for entry in entries {
        if native_field(entry, "loader")?.is_none() {
            continue;
        }
        managed_write_schema(
            &format!(
                "Intermediate loader '{}'",
                native_field(entry, "name")?.str()?
            ),
            &native_field(entry, "schema")?,
            (None, Some(&ignored)),
        )?;
    }
    Ok(())
}

fn names(value: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if value.is_none() {
        return Ok(Vec::new());
    }
    value.extract()
}

fn passed_defaults<'py>(arguments: &KeywordArguments<'py>) -> PyResult<LoaderDefaults<'py>> {
    Ok(LoaderDefaults {
        database: arguments.value("loader_default_database")?,
        schema: arguments.value("loader_default_schema")?,
    })
}

/// Select source and intermediate loader execution entries for `sqb load`.
#[pyfunction(name = "select_load_entries")]
#[pyo3(signature = (**options))]
fn select_load_entries_entry<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "select_load_entries",
        options,
        (
            &["discovered_inputs", "select", "exclude", "target_config"],
            &["loader_default_database", "loader_default_schema"],
        ),
    )?;
    let project: LoadProject<'py> = LoadProject::new(
        &arguments.value("discovered_inputs")?,
        &arguments.value("target_config")?,
    )?;
    let entries: Vec<Bound<'py, PyAny>> = project.selected_entries(
        (
            &names(&arguments.value("select")?)?,
            &names(&arguments.value("exclude")?)?,
        ),
        &passed_defaults(&arguments)?,
    )?;
    PyTuple::new(py, entries)
}

/// Unselected upstream intermediate loader entries the selected entries only reference.
#[pyfunction]
#[pyo3(signature = (**options))]
fn select_load_reference_entries<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "select_load_reference_entries",
        options,
        (
            &["discovered_inputs", "selected_sources", "target_config"],
            &["loader_default_database", "loader_default_schema"],
        ),
    )?;
    let project: LoadProject<'py> = LoadProject::new(
        &arguments.value("discovered_inputs")?,
        &arguments.value("target_config")?,
    )?;
    let selected: Vec<Bound<'py, PyAny>> = arguments
        .value("selected_sources")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    PyTuple::new(
        py,
        project.reference_entries(&selected, &passed_defaults(&arguments)?)?,
    )
}

/// The default database and schema intermediate loader tables are written to.
#[pyfunction]
#[pyo3(signature = (**options))]
fn resolve_loader_defaults<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "resolve_loader_defaults",
        options,
        (
            &["discovered_inputs"],
            &["selected_target", "target_config", "cli_vars"],
        ),
    )?;
    let defaults: LoaderDefaults<'py> = loader_defaults(
        &arguments.value("discovered_inputs")?,
        (
            &arguments.value("selected_target")?,
            &arguments.value("target_config")?,
            &arguments.value("cli_vars")?,
        ),
    )?;
    Ok((defaults.database, defaults.schema))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(select_load_entries_entry, module)?)?;
    module.add_function(wrap_pyfunction!(select_load_reference_entries, module)?)?;
    module.add_function(wrap_pyfunction!(resolve_loader_defaults, module)?)?;
    Ok(())
}
