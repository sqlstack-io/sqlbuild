//! Compile in the location interpreter and publish data through its scoped HTTP credential.

use sqlbuild_orchestration::platform::main::health::missing_provider_secrets::missing_provider_secrets;
use sqlbuild_orchestration::platform::models::{MissingProviderSecrets, PublishedGeneration};
use sqlbuild_python_front::discovery::main::sources::target_allowed_missing_secrets::target_allowed_missing_secrets;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::{Command, Stdio};

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::git_revisions::main::head_commit::head_commit;
use sqlbuild_orchestration::declarations::models::{
    Schedule, TriggerDeclaration, TriggerDefinition,
};
use sqlbuild_orchestration::dev_events::main::compile_summary::compile_summary;
use sqlbuild_orchestration::dev_events::main::emit_dev_steps::emit_dev_steps;
use sqlbuild_orchestration::dev_events::models::{DevPhase, DevStepState};
use sqlbuild_orchestration::platform::models::LocationRuleFailure;
use sqlbuild_orchestration::snapshots::models::SnapshotInput;
use sqlbuild_orchestration::warehouse_metadata::models::WarehouseStateLocation;
use sqlbuild_python_compile::provider_check::main::key_secret_blocks::key_secret_blocks;

use crate::bindings::_helpers::location_command::constants::EXECUTE_ACTION;
use crate::bindings::_helpers::location_command::constants::PUBLISH_ACTION;
use crate::bindings::_helpers::location_command::load_rules::load_rules;
use crate::bindings::_helpers::location_command::runtime::run_runtime;
use crate::bindings::_helpers::location_command::snapshot::build_orchestration_snapshot;
use crate::bindings::_helpers::location_command::snapshot::orchestration_snapshot;
use crate::bindings::_helpers::location_command::snapshot_connection::add_connection;
use crate::bindings::_helpers::location_command::types::HeldProject;
use crate::bindings::_helpers::location_command::worker::blocked_nodes;
use crate::bindings::_helpers::location_command::worker::execute_worker;
use crate::bindings::_helpers::query_command::project::ProjectAdapter;
use crate::bindings::_helpers::query_command::project::ProjectInputs;
use crate::bindings::_helpers::query_command::project::project_adapter;
use sqlbuild_orchestration::discovered_sets::models::DiscoveryPublishRequest;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::write_console::write_console;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::setup::compiled_phase::compiled_phase;
use sqlbuild_python_front::discovered_sets::main::discover_for_publication::discover_for_publication;
use sqlbuild_python_front::discovered_sets::main::set_discovery_source::set_discovery_source;
use sqlbuild_python_front::discovered_sets::main::with_set_retries::with_set_retries;
use sqlbuild_python_front::discovered_sets::models::DiscoverySource;
use sqlbuild_python_output::recall::main::write_stderr::write_stderr;
use sqlbuild_python_output::recall::models::InvocationRecall;
use sqlbuild_python_project::compiled_views::models::NativeProjectView;
use sqlbuild_python_project::project_snapshot::main::saved_discovered_sets::saved_discovered_sets;
use sqlbuild_python_project::project_snapshot::models::NativeOrchestrationSnapshot;
use sqlbuild_python_project::project_snapshot::types::NodeKeys;

fn status(py: Python<'_>, message: &str) -> PyResult<()> {
    write_stderr(py, &format!("{message}\n"))
}

/// Tell a `sqb dev` parent that each phase reached its state; silent outside `sqb dev`.
fn steps(
    py: Python<'_>,
    steps: &[(DevPhase, DevStepState)],
    detail: Option<String>,
) -> PyResult<()> {
    Ok(py.detach(|| emit_dev_steps(steps, detail))?)
}

/// Git is optional in deployed images; absent metadata is not fabricated from the last publication.
fn metadata(
    py: Python<'_>,
    arguments: &KeywordArguments<'_>,
    directory: &Path,
) -> PyResult<SnapshotInput> {
    let location: String = arguments.value("location_id")?.extract()?;
    let location_id = location
        .parse()
        .map_err(|_| PyValueError::new_err("location ID must be a UUID"))?;
    let target: String = arguments.value("selected_target")?.extract()?;
    let action = arguments.value("action")?.extract::<Option<String>>()?;
    let generation = arguments
        .value("generation")?
        .extract::<Option<i64>>()?
        .or_else(|| (action.as_deref() == Some(EXECUTE_ACTION)).then_some(1))
        .ok_or_else(|| PyValueError::new_err("publish requires --generation"))?;
    if target.trim().is_empty() || generation < 1 {
        return Err(PyValueError::new_err(
            "target must not be blank and generation must be positive",
        ));
    }
    let (git_revision, git_dirty, git_author) = py.detach(|| -> PyResult<_> {
        let revision = head_commit(directory);
        let mut dirty = false;
        let mut author = None;
        if revision.is_some() {
            for (is_dirty, options) in [
                (false, vec!["log", "-1", "--format=%an", "HEAD"]),
                (
                    true,
                    vec!["status", "--porcelain", "--untracked-files=normal"],
                ),
            ] {
                let output = Command::new("git")
                    .current_dir(directory)
                    .args(options)
                    .stdin(Stdio::null())
                    .output()?;
                if !output.status.success() {
                    return Err(PyRuntimeError::new_err(
                        "could not read location Git metadata",
                    ));
                }
                let text = String::from_utf8_lossy(&output.stdout);
                if is_dirty {
                    dirty = !text.trim().is_empty();
                } else {
                    author = Some(text.trim().to_owned());
                }
            }
        }
        Ok((revision, dirty, author))
    })?;
    Ok(SnapshotInput {
        location_id,
        target,
        generation,
        git_revision,
        git_dirty,
        git_author,
        image_id: arguments.value("image_id")?.extract()?,
        environment_hash: String::new(),
        sqb_version: String::new(),
        nodes: Default::default(),
        documents: Default::default(),
    })
}

/// A compiled snapshot, its blocked nodes and the compile's `CompiledProjectPhaseResult`.
type CompiledSnapshot<'py> = (NativeOrchestrationSnapshot, NodeKeys, Bound<'py, PyAny>);

fn compile_location<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
    input: &SnapshotInput,
) -> PyResult<(CompiledSnapshot<'py>, ProjectAdapter<'py>)> {
    status(py, "Discovering location project...")?;
    steps(py, &[(DevPhase::Discover, DevStepState::Started)], None)?;
    let project = project_adapter(
        py,
        Some(arguments.value("project_dir")?),
        ProjectInputs::DiscoveredDeferringSecrets,
    )?;
    status(py, "Location project discovered. Compiling snapshot...")?;
    let discovered = [
        (DevPhase::Discover, DevStepState::Finished),
        (DevPhase::Compile, DevStepState::Started),
    ];
    steps(py, &discovered, None)?;
    let compiled = compile_snapshot(py, arguments, (input, &project), None)?;
    status(py, "Location snapshot compiled.")?;
    let finished = [(DevPhase::Compile, DevStepState::Finished)];
    steps(py, &finished, Some(compile_summary(&compiled.0.input)))?;
    Ok((compiled, project))
}

/// Compile the discovered project into a snapshot, with a run's variables when it carries them.
pub(super) fn compile_snapshot<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
    (input, project): (&SnapshotInput, &ProjectAdapter<'py>),
    vars: Option<&Bound<'py, PyAny>>,
) -> PyResult<CompiledSnapshot<'py>> {
    let ProjectAdapter {
        project_dir,
        discovered,
        adapter,
    } = project;
    let mut options: Vec<(&str, Bound<'py, PyAny>)> =
        vec![("selected_target", arguments.value("selected_target")?)];
    if let Some(vars) = vars {
        options.push(("cli_vars", vars.clone()));
    }
    let options = new_record_with(py, RecordId::CompilePipelineOptions, &options)?;
    let compiled = compiled_phase(
        (discovered, adapter),
        &options,
        &py.None().into_bound(py),
        None,
    )?;
    let project = field(&compiled, "project")?;
    let view = project.cast::<NativeProjectView>()?.borrow();
    let snapshot = orchestration_snapshot(
        view,
        &serde_json::to_string(input).map_err(|error| PyValueError::new_err(error.to_string()))?,
        project_dir.extract()?,
        (
            discovered.clone(),
            !arguments
                .value("action")?
                .extract::<Option<String>>()?
                .is_some_and(|action| action == EXECUTE_ACTION),
        ),
    )?;
    let blocked = blocked_nodes(&project)?;
    Ok((snapshot, blocked, compiled))
}

pub(super) fn location_arguments<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<KeywordArguments<'py>> {
    KeywordArguments::read(
        py,
        "run_location_command",
        options,
        (
            &[
                "project_dir",
                "location_id",
                "selected_target",
                "generation",
                "server_url",
            ],
            &["image_id", "action", "run_id", "claim_id"],
        ),
    )
}

pub(super) fn publish_location<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
) -> PyResult<Result<(PublishedGeneration, HeldProject<'py>), LocationRuleFailure>> {
    if arguments
        .value("action")?
        .extract::<Option<String>>()?
        .is_some_and(|action| action != PUBLISH_ACTION)
    {
        return Err(PyValueError::new_err(
            "location serve must run through the native sqb executable",
        ));
    }
    let bearer = std::env::var("SQLBUILD_LOCATION_TOKEN").map_err(|_| {
        PyValueError::new_err("SQLBUILD_LOCATION_TOKEN must contain a UTF-8 location credential")
    })?;
    if bearer.trim().is_empty() {
        return Err(PyValueError::new_err(
            "SQLBUILD_LOCATION_TOKEN must not be blank",
        ));
    }
    let directory = arguments
        .value("project_dir")?
        .extract::<std::path::PathBuf>()?;
    let input = metadata(py, arguments, &directory)?;
    let ((mut snapshot, _, compiled), project) = compile_location(py, arguments, &input)?;
    if let Some(failure) = load_rules(py, arguments, (&project, &compiled))? {
        return Ok(Err(failure));
    }
    snapshot.input = add_connection(
        snapshot.input,
        (&project, &arguments.value("selected_target")?),
    )?;
    let locations: BTreeSet<_> = snapshot
        .input
        .nodes
        .values()
        .filter_map(|node| {
            let destination = &node.definition["destination"];
            Some(WarehouseStateLocation {
                database: destination["database"].as_str().map(str::to_owned),
                schema: destination["schema"].as_str()?.to_owned(),
            })
        })
        .collect();
    let triggers: BTreeMap<String, TriggerDeclaration> = serde_json::from_value(
        snapshot
            .input
            .documents
            .get("triggers")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    )
    .map_err(|error| PyValueError::new_err(error.to_string()))?;
    let refresh_at = triggers
        .values()
        .filter_map(|trigger| match trigger {
            TriggerDeclaration::Schedule(TriggerDefinition {
                schedule: Schedule::Calendar { refresh_at, .. },
                ..
            }) => Some(*refresh_at),
            TriggerDeclaration::Schedule(_) | TriggerDeclaration::Sensor(_) => None,
        })
        .min();
    let allowed: Vec<String> = target_allowed_missing_secrets(
        &field(&project.discovered, "project_config")?,
        arguments
            .value("selected_target")?
            .extract::<Option<String>>()?
            .as_deref(),
    )?;
    let missing_secrets: Vec<MissingProviderSecrets> = snapshot
        .input
        .documents
        .get("providers")
        .map(|providers| missing_provider_secrets(providers, &allowed))
        .unwrap_or_default();
    let mut secrets_needed_by: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if missing_secrets.iter().any(|provider| !provider.allowed) {
        let published: BTreeSet<String> = snapshot.input.nodes.keys().cloned().collect();
        let project_view: Bound<'py, PyAny> = field(&compiled, "project")?;
        for block in key_secret_blocks((&project.discovered, &project_view), &published)? {
            secrets_needed_by.insert(
                block.provider,
                block.users.into_iter().chain(block.readers).collect(),
            );
        }
    }
    status(py, "Publishing location snapshot...")?;
    steps(py, &[(DevPhase::Publish, DevStepState::Started)], None)?;
    let record = snapshot.publication(
            py,
            &arguments.value("server_url")?.extract::<String>()?,
            &bearer,
        )?.map_err(|error| {
            let message = if let sqlbuild_orchestration::api::errors::ClientError::Transport(transport) = &error {
                let cause = if transport.is_timeout() { " (the server did not answer in time)" } else { "" };
                format!("Publication outcome is unknown; generation {} may already be published. Check the current snapshot before retrying: {error}{cause}", input.generation)
            } else {
                error.to_string()
            };
            PyValueError::new_err(message)
        })?;
    status(py, "Location snapshot published.")?;
    steps(py, &[(DevPhase::Publish, DevStepState::Finished)], None)?;
    Ok(Ok((
        PublishedGeneration {
            record,
            refresh_at,
            state_locations: locations.into_iter().collect(),
            missing_secrets,
            secrets_needed_by,
        },
        (project.discovered, compiled),
    )))
}

/// One-shot publication and the supervisor's private stdio runtime share the same compiler path.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn run_location_command<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<i32> {
    run_location(py, options, None)
}

/// CLI dispatch retains the verified worker through invocation history delivery.
pub(crate) fn run_location<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
    recall: Option<&InvocationRecall>,
) -> PyResult<i32> {
    let arguments = location_arguments(py, options.clone())?;
    if arguments
        .value("action")?
        .extract::<Option<String>>()?
        .as_deref()
        == Some("execute")
    {
        let directory = arguments
            .value("project_dir")?
            .extract::<std::path::PathBuf>()?;
        let input = metadata(py, &arguments, &directory)?;
        let bearer = std::env::var("SQLBUILD_RUN_TOKEN")
            .map_err(|_| PyValueError::new_err("execute requires SQLBUILD_RUN_TOKEN"))?;
        let identity = (
            arguments.value("run_id")?.extract::<String>()?,
            arguments.value("claim_id")?.extract::<String>()?,
        );
        let server_url: String = arguments.value("server_url")?.extract()?;
        set_discovery_source(DiscoverySource::Saved(saved_discovered_sets(
            py,
            (&server_url, &bearer),
            (&identity.0, &identity.1),
        )?));
        let ((snapshot, blocked, compiled), project) = compile_location(py, &arguments, &input)?;
        return execute_worker(
            py,
            ((&snapshot, &compiled), project, blocked),
            (&arguments, &input),
            (&bearer, identity, recall),
        );
    }
    if arguments
        .value("action")?
        .extract::<Option<String>>()?
        .as_deref()
        == Some("runtime")
    {
        return run_runtime(py, options.unwrap_or_else(|| PyDict::new(py)));
    }
    let publication = (|| {
        discover_for_publication(
            &arguments,
            DiscoveryPublishRequest {
                generation: arguments
                    .value("generation")?
                    .extract::<Option<i64>>()?
                    .unwrap_or(1),
                rediscover: None,
                previous: Default::default(),
                seed: true,
            },
        )?;
        let (PublishedGeneration { record, .. }, _) =
            with_set_retries(|| publish_location(py, &arguments))?.map_err(|failure| {
                PyValueError::new_err(format!(
                    "Compile failed: {} rule findings",
                    failure.finding_count
                ))
            })?;
        let output = serde_json::to_string(&record)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        write_console(py, "stdout", &format!("{output}\n")).map_err(|error| {
            PyValueError::new_err(format!(
                "Snapshot {} was published, but result delivery failed: {error}",
                record.snapshot_id
            ))
        })?;
        Ok(0)
    })();
    if publication.is_err() {
        status(py, "Location command failed.")?;
    }
    publication
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_orchestration_snapshot, module)?)?;
    module.add_function(wrap_pyfunction!(run_location_command, module)?)
}
