//! Running a Python-free command: its checks, then its output with the observed services.

use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use sqlbuild_orchestration::api::errors::ClientError;
use sqlbuild_orchestration::platform::main::report_remote_location_health::report_remote_location_health;
use sqlbuild_orchestration::platform::models::{LocationHealth, LocationHealthReport};
use sqlbuild_orchestration::snapshots::main::next_remote_generation::next_remote_generation;
use sqlbuild_orchestration::snapshots::models::SnapshotRecord;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::MissedTickBehavior;
use uuid::Uuid;

use crate::argv::models::{Namespace, Stream, Value};
use crate::compute_logs::constants::DEFAULT_RETENTION_COUNT;
use crate::compute_logs::main::abandon_capture::abandon_capture;
use crate::compute_logs::main::finish_capture::finish_capture;
use crate::compute_logs::main::open_log_root::open_log_root;
use crate::compute_logs::main::start_capture::start_capture;
use crate::compute_logs::models::{CaptureEnd, CaptureStart, OpenCapture};
use crate::native_commands::_helpers::decision::{control_database, flag, text};
use crate::native_commands::_helpers::init::run_init;
use crate::native_commands::_helpers::package::installed_package;
use crate::native_commands::_helpers::paths::{location_database_files, project_location};
use crate::native_commands::_helpers::project_facts::has_no_runtime_extensions;
use crate::native_commands::_helpers::rules::run_rules;
use crate::native_commands::_helpers::skills::{run_skills, skill_freshness_notice};
use crate::native_commands::_helpers::terminal::{
    finish_lost_stdout, terminal_columns, write_output,
};
use crate::native_commands::constants::{
    COLUMNS_VARIABLE, FALLBACK_COLUMNS, UNCAUGHT_EXCEPTION_STATUS, UNKNOWN_COMMAND,
};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::main::_run_cost::run_cost;
use crate::native_commands::models::{
    CommandOutput, InstalledPackage, LocationCompileRequest, LocationCompiler, LocationServe,
    LostStdout, NativeCommand, NativeInvocation, ProjectLocation,
};
use crate::workers::main::open_local_workers::open_local_workers;
use crate::workers::models::LocalWorkers;

/// Request one publication without transferring interpreter ownership to a cancellable future.
pub(crate) async fn compile_generation(
    requests: &mpsc::Sender<LocationCompileRequest>,
    generation: i64,
    sources: &[sqlbuild_cache::project_snapshot::models::PathStamp],
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<bool>, ControlError> {
    let (reply, publication) = oneshot::channel();
    requests
        .send(LocationCompileRequest {
            generation,
            python_sources: sources
                .iter()
                .filter(|stamp| stamp.relative_path.ends_with(".py"))
                .cloned()
                .collect(),
            reply,
        })
        .await
        .map_err(|_| ControlError::Invalid("location compiler stopped".into()))?;
    tokio::pin!(stopped);
    let result = tokio::select! {
        result = &mut stopped => { result?; return Ok(None); },
        result = publication => result.map_err(|_| ControlError::Invalid("location compiler stopped before replying".into()))?,
    };
    let record = match result {
        Ok(record) => record,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!(
                    "Location compiler failed; generation {generation} may already be published: {error}\n"
                ),
            )]));
            None
        }
    };
    let published = record.is_some();
    if let Some(record) = record {
        let output = serde_json::to_string(&record)
            .map_err(|error| ControlError::Invalid(error.to_string()))?;
        if write_output(&[(Stream::Stdout, &format!("{output}\n"))]).is_some() {
            return Err(ControlError::Invalid(format!(
                "Snapshot {} was published, but result delivery failed",
                record.snapshot_id
            )));
        }
    }
    Ok(Some(published))
}

/// The child is retained by its owning task while this exchange can be cancelled.
pub(crate) async fn exchange_location_generation(
    compiler: &mut LocationCompiler,
    generation: i64,
) -> Result<Option<SnapshotRecord>, ControlError> {
    compiler
        .stdin
        .write_all(format!("{generation}\n").as_bytes())
        .await?;
    compiler.stdin.flush().await?;
    let mut line = String::new();
    if compiler.stdout.read_line(&mut line).await? == 0 {
        return Err(ControlError::Invalid(
            "location interpreter exited before replying".into(),
        ));
    }
    serde_json::from_str(&line).map_err(|_| {
        ControlError::Invalid("location interpreter returned an invalid publication record".into())
    })
}

/// Health IO remains native even while a user-code child holds its interpreter's GIL.
pub(crate) async fn report_location_health(
    options: &LocationServe,
    bearer: &str,
    session_id: Uuid,
    health: LocationHealth,
) {
    let report = LocationHealthReport {
        event_id: Uuid::now_v7(),
        session_id,
        target: options.target.clone(),
        health,
    };
    if let Err(error) =
        report_remote_location_health((&options.server_url, bearer), options.location_id, &report)
            .await
    {
        drop(write_output(&[(
            Stream::Stderr,
            &format!("Location health report failed: {error}\n"),
        )]));
    }
}

pub(crate) async fn location_heartbeats(
    options: &LocationServe,
    bearer: &str,
    session_id: Uuid,
) -> Result<(), ControlError> {
    let mut clock = tokio::time::interval(Duration::from_secs(30));
    clock.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        clock.tick().await;
        report_location_health(options, bearer, session_id, LocationHealth::Heartbeat {}).await;
    }
}

pub(crate) fn location_options(namespace: &Namespace) -> Result<LocationServe, ControlError> {
    if namespace
        .get("location_generation")
        .is_some_and(|value| *value != Value::None)
    {
        return Err(ControlError::Invalid(
            "serve derives --generation from the server; do not supply it".into(),
        ));
    }
    let project_dir = text(namespace, "project_dir")
        .map_or_else(std::env::current_dir, |directory| {
            std::fs::canonicalize(directory)
        })?;
    if !project_dir.is_dir() {
        return Err(ControlError::Invalid("project directory must exist".into()));
    }
    let location_id = text(namespace, "location_id")
        .unwrap_or_default()
        .parse()
        .map_err(|_| ControlError::Invalid("location ID must be a UUID".into()))?;
    let target = text(namespace, "target").unwrap_or_default();
    if target.trim().is_empty() {
        return Err(ControlError::Invalid("target must not be blank".into()));
    }
    Ok(LocationServe {
        project_dir,
        server_url: text(namespace, "location_server_url").unwrap_or_default(),
        location_id,
        target,
        image_id: text(namespace, "location_image_id"),
    })
}

/// Reconcile from the server after restarts or lost acknowledgements; transient outages retry.
pub(crate) async fn location_generation(
    options: &LocationServe,
    bearer: &str,
) -> Result<Option<i64>, ControlError> {
    match next_remote_generation(
        &options.server_url,
        bearer,
        options.location_id,
        &options.target,
    )
    .await
    {
        Ok(generation) => Ok(Some(generation)),
        Err(error) => {
            if matches!(&error, ClientError::Configuration(_))
                || matches!(&error, ClientError::Rejected(status) if matches!(status.as_u16(), 401 | 403))
            {
                return Err(ControlError::Invalid(error.to_string()));
            }
            drop(write_output(&[(
                Stream::Stderr,
                &format!(
                    "Location generation lookup failed; retaining last good snapshot and retrying: {error}\n"
                ),
            )]));
            Ok(None)
        }
    }
}

/// The installed interpreter owns user imports; the supervisor never imports Python.
pub(crate) fn location_runtime_child(options: &LocationServe, script: &Path) -> io::Result<Child> {
    let mut command = location_child(options, script);
    command
        .args([
            "location",
            "runtime",
            "--server",
            &options.server_url,
            "--location-id",
        ])
        .arg(options.location_id.to_string())
        .args(["--target", &options.target]);
    if let Some(image) = &options.image_id {
        command.args(["--image-id", image]);
    }
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).spawn()
}

/// Capture the ordinary debug check's exact local file paths without contaminating publication stdout.
pub(crate) async fn check_location_connection(
    options: &LocationServe,
    script: &Path,
    stopped: impl Future<Output = Result<(), ControlError>>,
) -> Result<Option<(bool, Vec<PathBuf>)>, ControlError> {
    let mut child = location_child(options, script)
        .args(["debug", "--target", &options.target, "--json"])
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| ControlError::Invalid("location debug output is unavailable".into()))?;
    let mut output: Vec<u8> = Vec::new();
    tokio::pin!(stopped);
    let connected = tokio::select! {
        result = &mut stopped => {
            child.kill().await?;
            result?;
            return Ok(None);
        },
        result = async { tokio::try_join!(child.wait(), stdout.read_to_end(&mut output)) } => result?.0.success(),
    };
    let files: Vec<String> = match serde_json::from_slice::<serde_json::Value>(&output)
        .and_then(|report| serde_json::from_value(report["local_files"].clone()))
    {
        Ok(files) => files,
        Err(_) => {
            drop(write_output(&[(
                Stream::Stderr,
                "Location connection check did not produce a valid report; retaining the published snapshot.\n",
            )]));
            return Ok(Some((false, Vec::new())));
        }
    };
    Ok(Some((
        connected,
        location_database_files(&options.project_dir, files),
    )))
}

fn location_child(options: &LocationServe, script: &Path) -> Command {
    let mut command = Command::new(script);
    command.arg("--project-dir").arg(&options.project_dir);
    if terminal_columns().is_err() {
        command.env(COLUMNS_VARIABLE, FALLBACK_COLUMNS.to_string());
    }
    command
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    command
}

/// Open and migrate the privileged store, reporting progress without contaminating JSON stdout.
pub(crate) async fn open_control_store(
    namespace: &Namespace,
) -> Result<ControlStore, ControlError> {
    let url = control_database(namespace)?;
    drop(write_output(&[(
        Stream::Stderr,
        "Opening control store...\n",
    )]));
    let store = open_store(&url).await?;
    migrate_store(&store).await?;
    drop(write_output(&[(Stream::Stderr, "Control store ready\n")]));
    Ok(store)
}

/// Decode privileged CLI configuration before handing process ownership to the workers domain.
pub(crate) fn open_control_workers(namespace: &Namespace) -> Result<LocalWorkers, ControlError> {
    open_local_workers(
        text(namespace, "control_launchers").as_deref(),
        text(namespace, "control_database_env")
            .unwrap_or_else(|| "SQLBUILD_CONTROL_DATABASE_URL".into()),
        !flag(namespace, "control_no_scheduler"),
    )
}

pub(crate) async fn shutdown_signal() -> Result<(), ControlError> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result?,
            _ = terminate.recv() => {},
        }
    }
    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;
    Ok(())
}

/// Stop sibling roles on a signal or role failure, then drain their shutdown results.
pub(crate) async fn supervise_control_roles(
    mut tasks: JoinSet<Result<(), ControlError>>,
    stop: watch::Sender<bool>,
) -> Result<(), ControlError> {
    let mut result = tokio::select! {
        result = tasks.join_next() => match result {
            Some(result) => result.map_err(ControlError::from).and_then(std::convert::identity),
            None => Ok(()),
        },
        result = shutdown_signal() => result,
    };
    stop.send_replace(true);
    while let Some(joined) = tasks.join_next().await {
        let joined = joined
            .map_err(ControlError::from)
            .and_then(std::convert::identity);
        result = result.and(joined);
    }
    result
}

/// The capture Python's `dispatch_with_compute_logs` opens; a failure only loses the log.
fn open_capture(
    invocation: &NativeInvocation,
    project: &ProjectLocation,
    started_at: SystemTime,
) -> io::Result<OpenCapture> {
    let retention: i64 = i64::try_from(DEFAULT_RETENTION_COUNT).unwrap_or(i64::MAX);
    let root: PathBuf = open_log_root(&project.given, None, Some(retention))
        .map_err(|error| io::Error::other(format!("{error:?}")))?;
    start_capture(CaptureStart {
        root,
        command: invocation
            .command_name
            .clone()
            .unwrap_or_else(|| UNKNOWN_COMMAND.to_owned()),
        project_dir: project.resolved.display().to_string(),
        target: invocation.target.clone(),
        started_at,
    })
}

/// Close the capture the way Python's does once the command's output met its reader.
fn close_capture(capture: OpenCapture, mut end: CaptureEnd, lost: Option<&LostStdout>) {
    match lost {
        None => drop(finish_capture(capture, &end)),
        Some(LostStdout::Raised(_)) => {
            end.stdout.clear();
            end.exit_code = i64::from(UNCAUGHT_EXCEPTION_STATUS);
            drop(finish_capture(capture, &end));
        }
        Some(LostStdout::AtExit(_) | LostStdout::Dropped) => drop(abandon_capture(capture, &end)),
    }
}

/// Print an observed command's output after the skill notice, retaining both in its capture.
fn finish_observed(
    invocation: &NativeInvocation,
    (project, package): (&ProjectLocation, &InstalledPackage),
    started_at: SystemTime,
    output: CommandOutput,
) -> i32 {
    let notice: String = if invocation.maintains_skills {
        skill_freshness_notice(&project.given, package)
    } else {
        String::new()
    };
    let capture: io::Result<OpenCapture> = open_capture(invocation, project, started_at);
    let stderr: String = notice + &output.stderr;
    let lost: Option<LostStdout> = write_output(&[
        (Stream::Stderr, stderr.as_str()),
        (Stream::Stdout, output.stdout.as_str()),
    ]);
    if let Ok(capture) = capture {
        close_capture(
            capture,
            CaptureEnd {
                stdout: output.stdout.into_bytes(),
                stderr: stderr.into_bytes(),
                exit_code: i64::from(output.exit_code),
            },
            lost.as_ref(),
        );
    }
    finish_lost_stdout(lost.as_ref(), output.exit_code)
}

/// What an observed command prints, unless it needs Python after all.
fn observed_output(
    command: &NativeCommand,
    project: &ProjectLocation,
    package: &InstalledPackage,
    error_color: bool,
) -> Result<CommandOutput, PythonNeeded> {
    if !has_no_runtime_extensions(&project.given) {
        return Err(PythonNeeded::Project);
    }
    match command {
        NativeCommand::Nothing => Ok(CommandOutput::default()),
        NativeCommand::Skills(options) => run_skills(&project.resolved, package, options),
        NativeCommand::Rules(options) => run_rules(&project.resolved, options),
        NativeCommand::Cost(options) => run_cost(&project.given, options, error_color),
        NativeCommand::Init => Err(PythonNeeded::Command),
    }
}

/// Run a parsed Python-free command; an error hands it to Python before any side effect.
pub(crate) fn run_command(
    invocation: &NativeInvocation,
    python_cli: &Path,
    error_color: bool,
) -> Result<i32, PythonNeeded> {
    let package: InstalledPackage = installed_package(python_cli)?;
    let project: ProjectLocation = project_location(invocation.project_dir.as_deref())?;
    if invocation.creates_project {
        let output: CommandOutput = run_init(&project.resolved, &package, error_color)?;
        let lost: Option<LostStdout> = write_output(&[
            (Stream::Stderr, output.stderr.as_str()),
            (Stream::Stdout, output.stdout.as_str()),
        ]);
        return Ok(finish_lost_stdout(lost.as_ref(), output.exit_code));
    }
    let started_at: SystemTime = SystemTime::now();
    let output: CommandOutput =
        observed_output(&invocation.command, &project, &package, error_color)?;
    Ok(finish_observed(
        invocation,
        (&project, &package),
        started_at,
        output,
    ))
}
