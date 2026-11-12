//! Project discovery: configuration, declaration files and imported project Python, validated.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PySet, PySetMethods, PyTuple};
use pyo3::{IntoPyObject, pyfunction, wrap_pyfunction};

use sqlbuild_host::entries::models::{ExecutorHost, UserCodeHost};

use crate::bindings::_helpers::boundary::python_calls::{
    KeywordArguments, call_with, keywords, within,
};
use crate::bindings::_helpers::compile_driver::config_validation::{
    ProjectConfiguration, reject_invalid_project_configuration,
};
use crate::bindings::_helpers::compile_driver::integration_loaders::integration_loader_functions;
use crate::bindings::_helpers::compile_driver::stage_work::{PROJECT_DISCOVERIES, count_stage};
use crate::bindings::_helpers::discovery::declaration_records::{
    discover_adapter_file, discover_audit_files, discover_constant_files, discover_enum_files,
    discover_macro_files, discover_model_schema_files, discover_seed_files,
    discover_sql_function_files, discover_sql_hook_files, retained_discovery_session,
};
use crate::bindings::_helpers::discovery::imported_providers::{
    discover_event_exporter_functions, discover_provider_classes,
};
use crate::bindings::_helpers::discovery::imported_python_nodes::{
    discover_hook_functions, discover_materialization_files, discover_python_node_functions,
};
use crate::bindings::_helpers::discovery::model_records::{
    ModelFileSelection, discovered_model_files,
};
use crate::bindings::_helpers::discovery::project_config::{
    load_local_config_file, load_project_config_file, warn_deprecated_settings,
};
use crate::bindings::_helpers::discovery::python_function_files::discover_python_function_files;
use crate::bindings::_helpers::discovery::python_paths::reject_unsupported_python_paths;
use crate::bindings::_helpers::discovery::snapshot::in_snapshot;
use crate::bindings::_helpers::discovery::sql_test_records::{
    discover_scenario_files, discover_test_files,
};
use crate::bindings::_helpers::discovery::yaml_records::{
    discover_schema_files, discover_source_files,
};
use crate::bindings::_helpers::event_exporting::scope::configure_discovered_event_exporters;
use crate::bindings::_helpers::frontier::compile_frontier::{FrontierStage, produced};
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use pyo3::IntoPyObjectExt;

const SQL_ANALYSIS_SETTING_KEY: &str = "sql_analysis";
/// The setting that turns delta compiles, and the project image they read, off.
const DELTA_COMPILE_SETTING_KEY: &str = "delta_compile";

/// Declaration file collections counted into the declaration-parse lifecycle, in field order.
const DECLARATION_COLLECTIONS: [&str; 14] = [
    "source_files",
    "model_files",
    "enum_files",
    "constant_files",
    "model_schema_files",
    "sql_function_files",
    "sql_hook_files",
    "python_function_files",
    "schema_files",
    "seed_files",
    "test_files",
    "scenario_files",
    "audit_files",
    "macro_files",
];

/// A native discoverer that takes only `project_dir`.
type FileDiscoverer = for<'py> fn(&Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>>;

/// Early discoverers by result field, in their original order.
const FILE_DISCOVERERS: [(&str, FileDiscoverer); 14] = [
    ("enum_files", |dir| {
        Ok(discover_enum_files(dir, None, false)?.into_any())
    }),
    ("constant_files", |dir| {
        Ok(discover_constant_files(dir, None, false)?.into_any())
    }),
    ("model_schema_files", |dir| {
        Ok(discover_model_schema_files(dir, None)?.into_any())
    }),
    ("sql_function_files", |dir| {
        Ok(discover_sql_function_files(dir, None)?.into_any())
    }),
    ("sql_hook_files", |dir| {
        Ok(discover_sql_hook_files(dir, None)?.into_any())
    }),
    ("python_function_files", |dir| {
        Ok(discover_python_function_files(dir)?.into_any())
    }),
    ("schema_files", |dir| {
        Ok(discover_schema_files(dir)?.into_any())
    }),
    ("seed_files", |dir| Ok(discover_seed_files(dir)?.into_any())),
    ("test_files", |dir| {
        Ok(discover_test_files(dir, None, None)?.into_any())
    }),
    ("scenario_files", |dir| {
        Ok(discover_scenario_files(dir, None)?.into_any())
    }),
    ("audit_files", |dir| {
        Ok(discover_audit_files(dir, None)?.into_any())
    }),
    ("macro_files", |dir| {
        Ok(discover_macro_files(dir, false)?.into_any())
    }),
    ("adapter_file", discover_adapter_file),
    ("native_session", retained_discovery_session),
];

/// Discoverers of imported project Python that take `project_dir` and `providers`.
const PYTHON_DISCOVERERS: [(&str, Discoverer); 5] = [
    (
        "python_nodes",
        Discoverer::Native(discover_python_node_functions),
    ),
    (
        "materialization_files",
        Discoverer::Native(discover_materialization_files),
    ),
    (
        "hook_functions",
        Discoverer::Native(discover_hook_functions),
    ),
    (
        "event_exporters",
        Discoverer::Native(discover_event_exporter_functions),
    ),
    (
        "command_output_sinks",
        Discoverer::Host(UserCodeHost::DiscoverCommandOutputSinkFunctions),
    ),
];

/// A native discoverer of project Python that takes `project_dir` and `providers`.
type PythonDiscoverer =
    for<'py> fn(Bound<'py, PyAny>, Option<Bound<'py, PyAny>>) -> PyResult<Bound<'py, PyAny>>;

/// Where a project-Python discoverer lives.
#[derive(Clone, Copy)]
enum Discoverer {
    Native(PythonDiscoverer),
    Host(UserCodeHost),
}

impl Discoverer {
    fn discovered<'py>(
        self,
        project_dir: &Bound<'py, PyAny>,
        providers: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        match self {
            Self::Native(discover) => discover(project_dir.clone(), Some(providers.clone())),
            Self::Host(entry) => call_with(
                &host::entry(project_dir.py(), entry)?,
                &[
                    ("project_dir", project_dir.clone()),
                    ("providers", providers.clone()),
                ],
            ),
        }
    }
}

/// Python node groups whose relative paths count as imported project Python.
const PYTHON_NODE_GROUPS: [&str; 5] = ["loaders", "tasks", "assets", "checks", "audit_factories"];

/// `OperationLifecycle(operation_kind="project", operation_name=name)`.
fn lifecycle<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    call_with(
        &host::entry(py, ExecutorHost::OperationLifecycle)?,
        &[
            ("operation_kind", "project".into_pyobject(py)?.into_any()),
            ("operation_name", name.into_pyobject(py)?.into_any()),
        ],
    )
}

/// The keyword arguments of one discovery, kept for the frontier stage.
struct DiscoveryRequest {
    project_dir: Py<PyAny>,
    sql_analysis_enabled_override: Py<PyAny>,
    extract_output_column_locations: bool,
}

/// Load all raw project inputs from disk before semantic resolution.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn discover_project_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "discover_project_inputs",
        options,
        (
            &["project_dir"],
            &[
                "sql_analysis_enabled_override",
                "extract_output_column_locations",
            ],
        ),
    )?;
    count_stage(py, PROJECT_DISCOVERIES, 1)?;
    let request: DiscoveryRequest = DiscoveryRequest {
        project_dir: arguments.value("project_dir")?.unbind(),
        sql_analysis_enabled_override: arguments.value("sql_analysis_enabled_override")?.unbind(),
        extract_output_column_locations: !arguments.passed("extract_output_column_locations")?
            || arguments.flag("extract_output_column_locations")?,
    };
    within(&lifecycle(py, "project_discovery")?, |_| {
        let assembled: Bound<'py, PyAny> =
            within(&lifecycle(py, "discovery_project_assembly")?, |_| {
                assembled_inputs(py, &request)
            })?;
        produced(FrontierStage::DiscoveredProjectInputs, assembled)
    })
}

/// `_assemble_discovered_project_inputs`: configuration, its checks, discovery and validation.
fn assembled_inputs<'py>(
    py: Python<'py>,
    request: &DiscoveryRequest,
) -> PyResult<Bound<'py, PyAny>> {
    let project_dir: &Bound<'py, PyAny> = request.project_dir.bind(py);
    let project_config: Bound<'py, PyAny> =
        load_project_config_file(py, project_dir)?.into_bound(py);
    let local_config: Bound<'py, PyAny> = load_local_config_file(py, project_dir)?.into_bound(py);
    reject_invalid_project_configuration(&ProjectConfiguration {
        project_dir,
        project_config: &project_config,
        local_config: &local_config,
    })?;
    warn_deprecated_settings(py, project_dir)?;
    reject_unsupported_python_paths(project_dir)?;
    let delta_settings: &Bound<'py, PyAny> =
        if native_field(&local_config, "setting_overrides")?.contains(DELTA_COMPILE_SETTING_KEY)? {
            &local_config
        } else {
            &project_config
        };
    if !native_field(
        &native_field(delta_settings, "settings")?,
        DELTA_COMPILE_SETTING_KEY,
    )?
    .is_truthy()?
    {
        crate::bindings::_helpers::compile_context::delta::stop_capture();
    }
    let sql_analysis_override: &Bound<'py, PyAny> = request.sql_analysis_enabled_override.bind(py);
    let sql_analysis_enabled: Bound<'py, PyAny> = if !sql_analysis_override.is_none() {
        sql_analysis_override.clone()
    } else if native_field(&local_config, "setting_overrides")?
        .contains(SQL_ANALYSIS_SETTING_KEY)?
    {
        native_field(&local_config, "settings")?.getattr("sql_analysis")?
    } else {
        native_field(&project_config, "settings")?.getattr("sql_analysis")?
    };
    let discovered: Bound<'py, PyAny> = discovered_inputs(
        py,
        (project_dir, &project_config, &local_config),
        (
            sql_analysis_enabled.is_truthy()?,
            request.extract_output_column_locations,
        ),
    )?;
    host::entry(py, UserCodeHost::ValidateDiscoveredInputs)?.call1((&discovered,))?;
    configure_discovered_event_exporters(&discovered)?;
    Ok(discovered)
}

/// Discover all project files and functions into one inputs bundle.
#[pyfunction]
#[pyo3(signature = (**options))]
fn build_discovered_project_inputs<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "build_discovered_project_inputs",
        options,
        (
            &[
                "project_dir",
                "project_config",
                "local_config",
                "sql_analysis_enabled",
            ],
            &["extract_output_column_locations"],
        ),
    )?;
    discovered_inputs(
        py,
        (
            &arguments.value("project_dir")?,
            &arguments.value("project_config")?,
            &arguments.value("local_config")?,
        ),
        (
            arguments.flag("sql_analysis_enabled")?,
            !arguments.passed("extract_output_column_locations")?
                || arguments.flag("extract_output_column_locations")?,
        ),
    )
}

/// `build_discovered_project_inputs` for a loaded configuration.
fn discovered_inputs<'py>(
    py: Python<'py>,
    (project_dir, project_config, local_config): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
    flags: (bool, bool),
) -> PyResult<Bound<'py, PyAny>> {
    let declarations: Bound<'py, PyDict> = within(
        &lifecycle(py, "discovery_declaration_parse")?,
        |declaration_lifecycle| {
            let declarations: Bound<'py, PyDict> = declaration_files(project_dir, flags)?;
            let mut item_count: usize = 0;
            for field in DECLARATION_COLLECTIONS {
                item_count += declarations.as_any().get_item(field)?.len()?;
            }
            item_count += usize::from(!declarations.as_any().get_item("adapter_file")?.is_none());
            completed(declaration_lifecycle, item_count)?;
            Ok(declarations)
        },
    )?;
    let python: Bound<'py, PyDict> = within(
        &lifecycle(py, "discovery_python_import")?,
        |python_lifecycle| {
            let python: Bound<'py, PyDict> = imported_python(project_dir)?;
            completed(python_lifecycle, imported_python_paths(&python)?)?;
            Ok(python)
        },
    )?;
    project_inputs(
        (project_dir, project_config, local_config),
        &declarations,
        &python,
    )
}

/// Report a lifecycle's completion with its item count.
fn completed(lifecycle: &Bound<'_, PyAny>, item_count: usize) -> PyResult<()> {
    let py: Python<'_> = lifecycle.py();
    let metadata: Bound<'_, PyDict> = keywords(
        py,
        &[("item_count", item_count.into_pyobject(py)?.into_any())],
    )?;
    call_with(
        &native_field(lifecycle, "completed")?,
        &[("metadata", metadata.into_any())],
    )?;
    Ok(())
}

/// `_discover_declaration_files`: every declaration file under one directory snapshot.
fn declaration_files<'py>(
    project_dir: &Bound<'py, PyAny>,
    (sql_analysis_enabled, extract_output_column_locations): (bool, bool),
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = project_dir.py();
    in_snapshot(project_dir, || {
        let files: Bound<'py, PyDict> = PyDict::new(py);
        files.set_item("source_files", discover_source_files(project_dir, None)?)?;
        files.set_item(
            "model_files",
            discovered_model_files(
                project_dir,
                (sql_analysis_enabled, extract_output_column_locations),
                ModelFileSelection {
                    selected_model_names: None,
                    on_fault: None,
                },
            )?,
        )?;
        for (field, discover) in FILE_DISCOVERERS {
            files.set_item(field, discover(project_dir)?)?;
        }
        Ok(files)
    })
}

/// Providers, then every kind of imported project Python that may use them.
fn imported_python<'py>(project_dir: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = project_dir.py();
    let providers: Bound<'py, PyAny> = discover_provider_classes(project_dir.clone())
        .and_then(|value| value.into_bound_py_any(py))?;
    let python: Bound<'py, PyDict> = PyDict::new(py);
    python.set_item("providers", &providers)?;
    for (field, discoverer) in PYTHON_DISCOVERERS {
        python.set_item(field, discoverer.discovered(project_dir, &providers)?)?;
    }
    Ok(python)
}

/// How many distinct project Python files the imported discoveries came from.
fn imported_python_paths(python: &Bound<'_, PyDict>) -> PyResult<usize> {
    let py: Python<'_> = python.py();
    let python: &Bound<'_, PyAny> = python.as_any();
    let paths: Bound<'_, PySet> = PySet::empty(py)?;
    let mut groups: Vec<Bound<'_, PyAny>> = vec![python.get_item("providers")?];
    let nodes: Bound<'_, PyAny> = python.get_item("python_nodes")?;
    for group in PYTHON_NODE_GROUPS {
        groups.push(native_field(&nodes, group)?);
    }
    for field in [
        "materialization_files",
        "hook_functions",
        "event_exporters",
        "command_output_sinks",
    ] {
        groups.push(python.get_item(field)?);
    }
    for group in groups {
        for item in group.try_iter()? {
            paths.add(native_field(&item?, "relative_path")?)?;
        }
    }
    Ok(paths.len())
}

/// The `DiscoveredProjectInputs` of the discovered declarations and imported Python.
fn project_inputs<'py>(
    (project_dir, project_config, local_config): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
    declarations: &Bound<'py, PyDict>,
    python: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let declarations: &Bound<'py, PyAny> = declarations.as_any();
    let python: &Bound<'py, PyAny> = python.as_any();
    let nodes: Bound<'py, PyAny> = python.get_item("python_nodes")?;
    let tuple_of = |group: &str| -> PyResult<Bound<'py, PyAny>> {
        Ok(PyTuple::new(
            py,
            native_field(&nodes, group)?
                .try_iter()?
                .collect::<PyResult<Vec<_>>>()?,
        )?
        .into_any())
    };
    let source_files: Bound<'py, PyAny> = declarations.get_item("source_files")?;
    let integration_loaders: Bound<'py, PyAny> = integration_loader_functions(py, &source_files)?;
    let loader_functions: Bound<'py, PyAny> = tuple_of("loaders")?.add(integration_loaders)?;
    let mut fields: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("project_config", project_config.clone()),
        ("local_config", local_config.clone()),
        ("project_dir", project_dir.clone()),
    ];
    for field in [
        "model_files",
        "enum_files",
        "constant_files",
        "model_schema_files",
        "sql_function_files",
        "sql_hook_files",
        "python_function_files",
        "schema_files",
        "source_files",
        "seed_files",
        "test_files",
        "scenario_files",
        "audit_files",
        "macro_files",
    ] {
        fields.push((field, declarations.get_item(field)?));
    }
    fields.push((
        "materialization_files",
        python.get_item("materialization_files")?,
    ));
    fields.push(("loader_functions", loader_functions));
    fields.push(("task_functions", tuple_of("tasks")?));
    fields.push(("asset_functions", tuple_of("assets")?));
    fields.push(("check_functions", tuple_of("checks")?));
    fields.push(("audit_factories", tuple_of("audit_factories")?));
    for field in [
        "hook_functions",
        "event_exporters",
        "command_output_sinks",
        "providers",
    ] {
        fields.push((field, python.get_item(field)?));
    }
    fields.push(("adapter_file", declarations.get_item("adapter_file")?));
    fields.push(("native_session", declarations.get_item("native_session")?));
    new_record_with(py, RecordId::DiscoveredProjectInputs, &fields)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(discover_project_inputs, module)?)?;
    module.add_function(wrap_pyfunction!(build_discovered_project_inputs, module)?)
}
