//! Provider classes and lifecycle-event sinks from imported providers/ and sinks/ modules.

use sqlbuild_core::constants::{EVENT_SINKS_PATH, PROVIDERS_PATH};
use crate::discovery::main::python_files::provider_by_name::provider_by_name;
use crate::discovery::main::python_files::provider_usages::provider_usages;
use std::collections::HashSet;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyType};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::discovery::_helpers::python_files::imported_modules::{
    ProjectDir, discovered, discovery_error, discovery_error_from, module_functions, shim, tuple_of,
};
use crate::discovery::_helpers::sources::provider_secrets::INVOCATION_TAKES_ALLOW_FLAG;
use crate::discovery::_helpers::sources::provider_secrets::TARGET_ALLOWED_MISSING_SECRETS;
use crate::discovery::constants::ALLOW_MISSING_SECRETS_FLAG;
use crate::discovery::models::SecretPolicy;
use sqlbuild_core::process_environment::main::env_file_shown::env_file_shown;
use sqlbuild_core::process_environment::main::shell_secret_fix::shell_secret_fix;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::compile_context::main::input_reads::record_settings_class_read::record_settings_class_read;
use sqlbuild_python_base::contracts::main::validate_resource_identity::validate_resource_identity;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use sqlbuild_python_base::orchestration_reads::main::provider_state::provider_state;
use sqlbuild_python_base::orchestration_reads::models::ProviderState;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get_or::mapping_get_or;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const PROVIDER_ERROR: ErrorId = ErrorId::ProviderDiscovery;
const SINK_ERROR: ErrorId = ErrorId::EventExporterDiscovery;
/// The provider name kept free so a sink's `RecallWriter` parameter can never be mistaken for a provider.
const RESERVED_PROVIDER_NAME: &str = "recall_writer";
const PROVIDER_INPUT_ERROR: UserCodeHost = UserCodeHost::ProviderInputError;
const SINK_DEFINITION: UserCodeHost = UserCodeHost::GetLifecycleEventSinkDefinition;
const ROOT_LOCATION: &str = "<root>";
const DEFAULT_MESSAGE: &str = "invalid value";
const DEFAULT_ERROR_TYPE: &str = "validation_error";
const DEFAULT_SETTINGS_ERROR: &str = "invalid provider settings";
const RECALL_WRITER_UNINSTALLED: &str = "the native module registered no RecallWriter class";

pub(crate) static RECALL_WRITER: PyOnceLock<Py<PyType>> = PyOnceLock::new();

fn reject_invalid_identity(
    name: &Bound<'_, PyAny>,
    relative_path: &Bound<'_, PyAny>,
) -> PyResult<()> {
    validate_resource_identity(name.downcast()?, "provider", relative_path, false)
}

/// The provider's validated name, from its explicit `provider_name` or `name()`.
fn provider_name<'py>(
    provider_class: &Bound<'py, PyAny>,
    relative_path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = provider_class.py();
    let explicit: Bound<'py, PyAny> = native_field(provider_class, "provider_name")?;
    if !explicit.is_none() {
        reject_invalid_identity(&explicit, relative_path)?;
    }
    let name: Bound<'py, PyAny> = match provider_class.call_method0("name") {
        Ok(name) => name,
        Err(error) => {
            let input_error: Bound<'py, PyAny> = host_entry(py, PROVIDER_INPUT_ERROR)?;
            if !error.value(py).is_instance(&input_error)? {
                return Err(error);
            }
            let message: String = format!(
                "Provider class {} in {} has an invalid provider name: {}",
                provider_class.getattr("__name__")?.str()?,
                relative_path.str()?,
                error.value(py)
            );
            return Err(discovery_error_from(py, (PROVIDER_ERROR, message), error));
        }
    };
    reject_invalid_identity(&name, relative_path)?;
    Ok(name)
}

/// `"<location>: <message> [<type>]"` per pydantic error, one per line.
fn validation_details(error: &Bound<'_, PyAny>) -> PyResult<String> {
    let py: Python<'_> = error.py();
    let options: Bound<'_, PyDict> = PyDict::new(py);
    options.set_item("include_input", false)?;
    let mut details: Vec<String> = Vec::new();
    for item in error
        .call_method("errors", (), Some(&options))?
        .try_iter()?
    {
        let item: Bound<'_, PyAny> = item?;
        let location: Bound<'_, PyAny> = mapping_get_or(&item, "loc", py.None())?;
        let location_text: String = if location.is_none() || !location.is_truthy()? {
            ROOT_LOCATION.to_owned()
        } else {
            location
                .try_iter()?
                .map(|part| part?.str()?.extract::<String>())
                .collect::<PyResult<Vec<String>>>()?
                .join(".")
        };
        let message: Bound<'_, PyAny> = mapping_get_or(&item, "msg", DEFAULT_MESSAGE)?;
        let error_type: Bound<'_, PyAny> = mapping_get_or(&item, "type", DEFAULT_ERROR_TYPE)?;
        details.push(format!(
            "{location_text}: {} [{}]",
            message.str()?,
            error_type.str()?
        ));
    }
    if details.is_empty() {
        return Ok(DEFAULT_SETTINGS_ERROR.to_owned());
    }
    Ok(details.join("\n"))
}

/// An instance of the provider class, with missing secrets an error unless `policy` defers or allows them.
fn provider_instance<'py>(
    provider_class: &Bound<'py, PyAny>,
    (provider_name, relative_path): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    policy: SecretPolicy,
) -> PyResult<(Bound<'py, PyAny>, Vec<String>)> {
    let py: Python<'py> = provider_class.py();
    let missing: Vec<String> = match provider_state(provider_class)? {
        ProviderState::Ready(instance) => return Ok((instance, Vec::new())),
        ProviderState::MissingSecrets(missing) => missing,
        ProviderState::Invalid(error) => {
            let message: String = format!(
                "Provider '{}' in {} has invalid settings:\n{}",
                provider_name.str()?,
                relative_path.str()?,
                validation_details(error.value(py))?
            );
            return Err(discovery_error_from(py, (PROVIDER_ERROR, message), error));
        }
    };
    let name: String = provider_name.extract()?;
    let target_allows: bool =
        TARGET_ALLOWED_MISSING_SECRETS.with_borrow(|(_, providers)| providers.contains(&name));
    if policy == SecretPolicy::Require && !target_allows {
        let variables: String = missing.join(", ");
        let noun: &str = if missing.len() == 1 {
            "secret"
        } else {
            "secrets"
        };
        let error: PyErr = discovery_error(
            py,
            PROVIDER_ERROR,
            format!(
                "provider '{}' ({}) is missing {noun} {variables}",
                provider_name.str()?,
                relative_path.str()?
            ),
        );
        let mut help: String = shell_secret_fix(&variables, env_file_shown().as_deref());
        if INVOCATION_TAKES_ALLOW_FLAG.get() {
            help.push_str(&format!(
                ", or pass {ALLOW_MISSING_SECRETS_FLAG} where secrets aren't available"
            ));
        }
        error.value(py).setattr("help", help)?;
        return Err(error);
    }
    Ok((provider_class.call_method0("model_construct")?, missing))
}

/// Provider classes under providers/.
#[pyfunction]
#[pyo3(signature = (*, project_dir, secret_policy = SecretPolicy::Require.name()))]
pub(crate) fn discover_provider_classes<'py>(
    project_dir: Bound<'py, PyAny>,
    secret_policy: &str,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let policy: SecretPolicy = SecretPolicy::parse(secret_policy)?;
    let mut allowed: Vec<String> = Vec::new();
    let project = ProjectDir { path: project_dir };
    if let Some(cached) = project.cached_extensions()? {
        let providers: Bound<'py, PyAny> = cached.get_item(0)?;
        for provider in providers.try_iter()? {
            let provider: Bound<'py, PyAny> = provider?;
            let provider_class: Bound<'py, PyAny> = native_field(&provider, "provider_class")?;
            record_settings_class_read(&provider_class)?;
            if policy == SecretPolicy::Defer {
                continue;
            }
            let name: Bound<'py, PyAny> = native_field(&provider, "name")?;
            let (_, missing) = provider_instance(
                &provider_class,
                (&name, &native_field(&provider, "relative_path")?),
                policy,
            )?;
            if !missing.is_empty() {
                allowed.push(name.extract()?);
            }
        }
        report_allowed(py, policy, &allowed)?;
        return Ok(providers);
    }
    if !project
        .child(PROVIDERS_PATH)?
        .call_method0("is_dir")?
        .is_truthy()?
    {
        return tuple_of(py, Vec::new());
    }
    let mut providers: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut seen: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
    for file_path in project.module_files(PROVIDERS_PATH, true)? {
        let module: Bound<'py, PyAny> =
            project.load(UserCodeHost::LoadProviderModule, &file_path)?;
        let classes: Bound<'py, PyAny> =
            shim(py, UserCodeHost::ModuleProviderClasses)?.call1((&module,))?;
        for provider_class in classes.try_iter()? {
            let provider_class: Bound<'py, PyAny> = provider_class?;
            let relative_path: Bound<'py, PyAny> = project.relative(&file_path)?;
            let name: Bound<'py, PyAny> = provider_name(&provider_class, &relative_path)?;
            let name_text: String = name.extract()?;
            if name_text == RESERVED_PROVIDER_NAME {
                return Err(discovery_error(
                    py,
                    PROVIDER_ERROR,
                    format!(
                        "Provider name '{RESERVED_PROVIDER_NAME}' in {} is reserved for the \
                         RecallWriter of Recall replacement sinks; rename the provider",
                        project.relative_to(&file_path)?
                    ),
                ));
            }
            if let Some((_, existing)) = seen.iter().find(|(listed, _)| *listed == name_text) {
                return Err(discovery_error(
                    py,
                    PROVIDER_ERROR,
                    format!(
                        "Duplicate provider name '{name_text}' found in {} and {}",
                        project.relative_to(existing)?,
                        project.relative_to(&file_path)?
                    ),
                ));
            }
            seen.push((name_text, file_path.clone()));
            record_settings_class_read(&provider_class)?;
            let (settings, missing) =
                provider_instance(&provider_class, (&name, &relative_path), policy)?;
            if !missing.is_empty() {
                allowed.push(name.extract()?);
            }
            providers.push(discovered(
                py,
                RecordId::DiscoveredProvider,
                &[
                    ("file_path", file_path.clone()),
                    ("relative_path", relative_path),
                    ("name", name),
                    ("provider_class", provider_class),
                    ("settings", settings),
                ],
            )?);
        }
    }
    report_allowed(py, policy, &allowed)?;
    tuple_of(py, providers)
}

/// Note on stderr the providers running with missing secrets: all under `Allow`, the target's listed ones under `Require`.
fn report_allowed(py: Python<'_>, policy: SecretPolicy, allowed: &[String]) -> PyResult<()> {
    if policy == SecretPolicy::Defer || allowed.is_empty() {
        return Ok(());
    }
    let why: String = match TARGET_ALLOWED_MISSING_SECRETS.with_borrow(|(target, _)| target.clone())
    {
        Some(target) if policy == SecretPolicy::Require => format!("allowed on target {target}"),
        _ => "allowed".to_owned(),
    };
    let noun: &str = if allowed.len() == 1 {
        "provider has"
    } else {
        "providers have"
    };
    write_console(
        py,
        "stderr",
        &format!(
            "{} {noun} missing secrets ({why}): {}\n",
            allowed.len(),
            allowed.join(", ")
        ),
    )
}

/// Sink modules imported and their lifecycle declarations collected, without providers.
#[pyfunction]
#[pyo3(signature = (*, project_dir))]
pub(crate) fn discover_event_exporter_declarations<'py>(
    project_dir: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project = ProjectDir { path: project_dir };
    if !project
        .child(EVENT_SINKS_PATH)?
        .call_method0("is_dir")?
        .is_truthy()?
    {
        return tuple_of(py, Vec::new());
    }
    let read_definition: Bound<'py, PyAny> = host_entry(py, SINK_DEFINITION)?;
    let mut declarations: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut seen: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
    for file_path in project.module_files(EVENT_SINKS_PATH, true)? {
        let module: Bound<'py, PyAny> = project.load(UserCodeHost::LoadEventSinkModule, &file_path)?;
        let mut seen_functions: HashSet<usize> = HashSet::new();
        for function in module_functions(&module)? {
            if !seen_functions.insert(function.as_ptr() as usize) {
                continue;
            }
            let read: Bound<'py, PyAny> = read_definition.call1((&function,))?;
            if read.is_none() {
                continue;
            }
            let name: String = native_field(&read, "name")?.extract()?;
            if let Some((_, existing)) = seen.iter().find(|(listed, _)| *listed == name) {
                return Err(discovery_error(
                    py,
                    SINK_ERROR,
                    format!(
                        "Duplicate event exporter name '{name}' found in {} and {}",
                        project.relative_to(existing)?,
                        project.relative_to(&file_path)?
                    ),
                ));
            }
            let relative_path: Bound<'py, PyAny> = project.relative(&file_path)?;
            call_with(
                &shim(py, UserCodeHost::ValidateEventExporterSignature)?,
                &[
                    ("function", function.clone()),
                    ("exporter_name", native_field(&read, "name")?),
                    ("relative_path", relative_path.clone()),
                ],
            )?;
            seen.push((name, file_path.clone()));
            declarations.push(discovered(
                py,
                RecordId::DiscoveredEventExporterDeclaration,
                &[
                    ("file_path", file_path.clone()),
                    ("relative_path", relative_path),
                    ("name", native_field(&read, "name")?),
                    ("function", function.clone()),
                    ("event_kinds", native_field(&read, "event_kinds")?),
                    ("min_severity", native_field(&read, "min_severity")?),
                ],
            )?);
        }
    }
    tuple_of(py, declarations)
}

/// Each parameter of `function` with its annotation, and whether that annotation is `RecallWriter`.
fn annotated_parameters<'py>(
    function: &Bound<'py, PyAny>,
) -> PyResult<Vec<(String, Bound<'py, PyAny>, bool)>> {
    let py: Python<'py> = function.py();
    let writer: &Py<PyType> = RECALL_WRITER
        .get(py)
        .ok_or_else(|| PyRuntimeError::new_err(RECALL_WRITER_UNINSTALLED))?;
    let mut parameters: Vec<(String, Bound<'py, PyAny>, bool)> = Vec::new();
    for pair in shim(py, UserCodeHost::ParameterAnnotations)?
        .call1((function,))?
        .try_iter()?
    {
        let (parameter, annotation): (String, Bound<'py, PyAny>) = pair?.extract()?;
        let is_writer: bool = annotation.is(writer.bind(py).as_any());
        parameters.push((parameter, annotation, is_writer));
    }
    Ok(parameters)
}

/// Whether `declaration`'s function takes a `RecallWriter`, making it a Recall replacement sink.
pub(crate) fn takes_recall_writer(declaration: &Bound<'_, PyAny>) -> PyResult<bool> {
    Ok(
        annotated_parameters(&native_field(declaration, "function")?)?
            .iter()
            .any(|(_, _, is_writer)| *is_writer),
    )
}

/// Provider parameters after the event, each naming a known provider of the right class.
fn exporter_provider_usages<'py>(
    declaration: &Bound<'py, PyAny>,
    (project, providers): (&ProjectDir<'py>, &Bound<'py, PyDict>),
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project.py();
    let function: Bound<'py, PyAny> = native_field(declaration, "function")?;
    let name: String = native_field(declaration, "name")?.extract()?;
    let relative_path: String = project
        .relative(&native_field(declaration, "file_path")?)?
        .str()?
        .extract()?;
    let empty: Bound<'py, PyAny> = imported(py, "inspect", "Parameter")?.getattr("empty")?;
    for (parameter, annotation, is_writer) in annotated_parameters(&function)?.into_iter().skip(1) {
        if is_writer {
            continue;
        }
        let Some(provider) = providers.get_item(&parameter)? else {
            return Err(discovery_error(
                py,
                SINK_ERROR,
                format!(
                    "Event exporter '{name}' in {relative_path} requires unknown provider '{parameter}'"
                ),
            ));
        };
        let provider_class: Bound<'py, PyAny> = native_field(&provider, "provider_class")?;
        if !annotation.is(&empty) && !annotation.is(&provider_class) {
            return Err(discovery_error(
                py,
                SINK_ERROR,
                format!(
                    "Event exporter '{name}' in {relative_path} provider parameter '{parameter}' must be unannotated or exactly {}",
                    provider_class.getattr("__name__")?.str()?
                ),
            ));
        }
    }
    provider_usages(&function, providers)
}

/// Declarations bound to the discovered project providers they name.
#[pyfunction]
#[pyo3(signature = (*, declarations, providers, project_dir))]
pub(crate) fn bind_event_exporter_declarations<'py>(
    declarations: Bound<'py, PyAny>,
    providers: Bound<'py, PyAny>,
    project_dir: Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = project_dir.py();
    let project = ProjectDir { path: project_dir };
    let providers: Bound<'py, PyDict> = provider_by_name(&providers)?;
    let mut bound: Vec<Bound<'py, PyAny>> = Vec::new();
    for declaration in declarations.try_iter()? {
        let declaration: Bound<'py, PyAny> = declaration?;
        let usages: Bound<'py, PyAny> =
            exporter_provider_usages(&declaration, (&project, &providers))?;
        let mut pairs: Vec<(&str, Bound<'py, PyAny>)> = Vec::new();
        for field in [
            "file_path",
            "relative_path",
            "name",
            "function",
            "event_kinds",
            "min_severity",
        ] {
            pairs.push((field, native_field(&declaration, field)?));
        }
        pairs.push(("provider_usages", usages));
        bound.push(discovered(py, RecordId::DiscoveredEventExporter, &pairs)?);
    }
    tuple_of(py, bound)
}
