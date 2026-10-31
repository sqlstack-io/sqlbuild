//! Running a Python-free command: its checks, then its output with the observed services.

use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::main::migrate_store::migrate_store;
use sqlbuild_orchestration::store::main::open_store::open_store;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::sync::watch;
use tokio::task::JoinSet;

use crate::argv::models::{Namespace, Stream};
use crate::compute_logs::constants::DEFAULT_RETENTION_COUNT;
use crate::compute_logs::main::abandon_capture::abandon_capture;
use crate::compute_logs::main::finish_capture::finish_capture;
use crate::compute_logs::main::open_log_root::open_log_root;
use crate::compute_logs::main::start_capture::start_capture;
use crate::compute_logs::models::{CaptureEnd, CaptureStart, OpenCapture};
use crate::native_commands::_helpers::decision::control_database;
use crate::native_commands::_helpers::init::run_init;
use crate::native_commands::_helpers::package::installed_package;
use crate::native_commands::_helpers::paths::project_location;
use crate::native_commands::_helpers::project_facts::has_no_runtime_extensions;
use crate::native_commands::_helpers::rules::run_rules;
use crate::native_commands::_helpers::skills::{run_skills, skill_freshness_notice};
use crate::native_commands::_helpers::terminal::{finish_lost_stdout, write_output};
use crate::native_commands::constants::{UNCAUGHT_EXCEPTION_STATUS, UNKNOWN_COMMAND};
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::main::_run_cost::run_cost;
use crate::native_commands::models::{
    CommandOutput, InstalledPackage, LostStdout, NativeCommand, NativeInvocation, ProjectLocation,
};

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

async fn shutdown_signal() -> Result<(), ControlError> {
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
