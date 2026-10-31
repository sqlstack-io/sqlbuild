//! Compile in the location interpreter and publish data through its scoped HTTP credential.

use std::path::Path;
use std::process::{Command, Stdio};

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::git_revisions::main::head_commit::head_commit;
use sqlbuild_orchestration::snapshots::models::{SnapshotInput, SnapshotRecord};

use crate::bindings::_helpers::boundary::python_calls::KeywordArguments;
use crate::bindings::_helpers::compile_driver::pipeline::compiled_phase;
use crate::bindings::_helpers::compiled_project::snapshot::{
    NativeOrchestrationSnapshot, build_orchestration_snapshot,
};
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;
use crate::bindings::_helpers::location_command::constants::PUBLISH_ACTION;
use crate::bindings::_helpers::location_command::runtime::run_runtime;
use crate::bindings::_helpers::location_command::worker::execute_worker;
use crate::bindings::_helpers::native_host::host::write_console;
use crate::bindings::_helpers::query_command::project::{
    ProjectAdapter, ProjectInputs, project_adapter,
};
use crate::bindings::_helpers::recall::session::InvocationRecall;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::ids::RecordId;

fn status(py: Python<'_>, message: &str) -> PyResult<()> {
    write_console(py, "stderr", &format!("{message}\n"))
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
        .or_else(|| (action.as_deref() == Some("execute")).then_some(1))
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

fn compile_location<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
    input: &SnapshotInput,
) -> PyResult<(NativeOrchestrationSnapshot, Bound<'py, PyAny>)> {
    status(py, "Discovering location project...")?;
    let ProjectAdapter {
        project_dir,
        discovered,
        adapter,
    } = project_adapter(
        py,
        Some(arguments.value("project_dir")?),
        ProjectInputs::Discovered,
    )?;
    status(py, "Location project discovered. Compiling snapshot...")?;
    let options = new_record_with(
        py,
        RecordId::CompilePipelineOptions,
        &[("selected_target", arguments.value("selected_target")?)],
    )?;
    let compiled = compiled_phase((&discovered, &adapter), &options, &py.None().into_bound(py))?;
    let project = field(&compiled, "project")?;
    let view = project.cast::<NativeProjectView>()?.borrow();
    let snapshot = build_orchestration_snapshot(
        view,
        &serde_json::to_string(input).map_err(|error| PyValueError::new_err(error.to_string()))?,
        project_dir.extract()?,
        None,
    )?;
    status(py, "Location snapshot compiled.")?;
    Ok((snapshot, discovered))
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

pub(super) fn publish_location(
    py: Python<'_>,
    arguments: &KeywordArguments<'_>,
) -> PyResult<SnapshotRecord> {
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
    let (snapshot, _) = compile_location(py, arguments, &input)?;
    status(py, "Publishing location snapshot...")?;
    let record = snapshot.publication(
            py,
            &arguments.value("server_url")?.extract::<String>()?,
            &bearer,
        )?.map_err(|error| {
            let message = if matches!(error, sqlbuild_orchestration::api::errors::ClientError::Transport(_)) {
                format!("Publication outcome is unknown; generation {} may already be published. Check the current snapshot before retrying: {error}", input.generation)
            } else {
                error.to_string()
            };
            PyValueError::new_err(message)
        })?;
    status(py, "Location snapshot published.")?;
    Ok(record)
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
        let (snapshot, discovered) = compile_location(py, &arguments, &input)?;
        return execute_worker(
            py,
            (&snapshot, discovered),
            &arguments,
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
        let record = publish_location(py, &arguments)?;
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
    module.add_function(wrap_pyfunction!(run_location_command, module)?)
}
