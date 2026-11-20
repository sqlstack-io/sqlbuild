//! Collapse the production API, scheduler and outbound location into one local process tree.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Instant;

use sqlbuild_core::screen_text::models::{Problem, ProblemLevel, ResultLine};
use sqlbuild_orchestration::store::errors::ControlError;

use crate::argv::models::Namespace;
use crate::dev_screen::main::start_dev_screen::start_dev_screen;
use crate::dev_screen::models::{DevScreenHandle, DevStep, ScreenEnd, ScreenMessage};
use crate::dev_screen::types::DevScreenParts;
use crate::dev_stop::constants::{RUN_FAILURE_STATUS, START_FAILURE_STATUS};
use crate::dev_stop::main::stop_switch::stop_switch;
use crate::dev_stop::main::watch_signals::watch_signals;
use crate::dev_stop::models::DevEnd;
use crate::native_commands::_helpers::command::decision::{flag, text};
use crate::native_commands::_helpers::command::project_facts::development_config;
use crate::native_commands::_helpers::location::execution::run_development_roles;
use crate::native_commands::_helpers::location::execution::{
    announced_store, loopback_address, screen_setup,
};

/// Run `sqb dev` until it is stopped or fails, leave its result line, and return its exit status.
pub(super) async fn run_dev(namespace: &Namespace) -> i32 {
    let started: Instant = Instant::now();
    let project: PathBuf =
        match std::fs::canonicalize(text(namespace, "project_dir").unwrap_or_else(|| ".".into())) {
            Ok(project) => project,
            Err(error) => {
                eprintln!("\u{2717} sqb dev did not start  project directory: {error}");
                return START_FAILURE_STATUS;
            }
        };
    if let Err(error) = watch_signals() {
        eprintln!("\u{2717} sqb dev did not start  signals: {error}");
        return START_FAILURE_STATUS;
    }
    let config = development_config(&project, text(namespace, "target"));
    let (target, adapter): (String, String) = config.clone().unwrap_or_default();
    let DevScreenHandle {
        messages,
        actions,
        keys,
        finish,
        task,
    } = start_dev_screen(screen_setup(
        &project,
        (&target, &adapter),
        flag(namespace, "development_plain"),
    ));
    let end: DevEnd = match config {
        Err(problem) => {
            let file: String = problem.title.clone();
            drop(messages.send(ScreenMessage::Failed {
                step: DevStep::Config,
                detail: "1 error".to_owned(),
                problem,
            }));
            DevEnd::NotStarted {
                reason: format!("configuration error in {file}"),
                explained: true,
                next: vec![(
                    "sqb debug".to_owned(),
                    "check the configuration on its own".to_owned(),
                )],
            }
        }
        Ok(_) => start_roles(namespace, (project, target), (messages.clone(), actions, keys)).await,
    };
    let (status, result): (i32, ResultLine) = result_line(&end, started);
    let show_log: bool = matches!(&end, DevEnd::NotStarted { explained: false, .. })
        || matches!(&end, DevEnd::Failed { .. });
    drop(finish.send(ScreenEnd { result, show_log }));
    drop(task.await);
    stop_switch().kill_all();
    stop_switch().restore_terminal();
    status
}

/// Open the store and the port, each a step that can fail, then run every role.
async fn start_roles(
    namespace: &Namespace,
    (project, target): (PathBuf, String),
    (messages, actions, keys): DevScreenParts,
) -> DevEnd {
    let not_started = |step: DevStep, title: &str, (why, fix): (String, &str)| {
        drop(messages.send(ScreenMessage::Failed {
            step,
            detail: "failed".to_owned(),
            problem: Problem {
                level: ProblemLevel::Error,
                title: title.to_owned(),
                code: String::new(),
                at: Vec::new(),
                why: why.clone(),
                fix: fix.to_owned(),
            },
        }));
        DevEnd::NotStarted {
            reason: format!("could not {title_verb}", title_verb = verb(step)),
            explained: true,
            next: Vec::new(),
        }
    };
    let address: SocketAddr = match loopback_address(namespace) {
        Ok(address) => address,
        Err(error) => {
            return not_started(
                DevStep::Api,
                "UI server address",
                (message(error), "pass --port with a free port on 127.0.0.1"),
            );
        }
    };
    let executable: PathBuf = match std::env::current_exe() {
        Ok(executable) => executable,
        Err(error) => {
            return DevEnd::NotStarted {
                reason: format!("could not find the sqb executable: {error}"),
                explained: false,
                next: Vec::new(),
            };
        }
    };
    let (store, scope) = match announced_store(namespace, (&project, &target), &messages).await {
        Ok(opened) => opened,
        Err(error) => {
            return not_started(
                DevStep::ControlStore,
                "control store",
                (
                    message(error),
                    "check --control-store-url, or remove .sqlbuild/control.sqlite3 if it is damaged",
                ),
            );
        }
    };
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            return not_started(
                DevStep::Api,
                "UI server",
                (
                    format!("could not listen on {address}: {error}"),
                    "stop whatever uses the port, or pick another with --port (0 picks a free one)",
                ),
            );
        }
    };
    match run_development_roles(
        (store, scope),
        (project, executable),
        listener,
        (
            !flag(namespace, "control_no_scheduler"),
            (messages.clone(), actions, keys),
        ),
    )
    .await
    {
        Ok(end) => end,
        Err(error) => DevEnd::NotStarted {
            reason: message(error),
            explained: false,
            next: Vec::new(),
        },
    }
}

/// What a failed step could not do, for the result line.
fn verb(step: DevStep) -> &'static str {
    match step {
        DevStep::ControlStore => "open the control store",
        DevStep::Api => "start the UI server",
        _ => "start",
    }
}

/// A control error's own words, without the category prefix.
fn message(error: ControlError) -> String {
    match error {
        ControlError::Invalid(message) => message,
        error => error.to_string(),
    }
}

/// The exit status and result line for how the session ended.
fn result_line(end: &DevEnd, started: Instant) -> (i32, ResultLine) {
    match end {
        DevEnd::Stopped(reason) => (
            reason.status(),
            ResultLine {
                failed: false,
                title: format!("sqb dev stopped{}", reason.note()),
                detail: format!("ran {}", ran(started)),
                elapsed: stop_switch().stopping_for(),
                next: Vec::new(),
            },
        ),
        DevEnd::NotStarted { reason, next, .. } => (
            START_FAILURE_STATUS,
            ResultLine {
                failed: true,
                title: "sqb dev did not start".to_owned(),
                detail: reason.clone(),
                elapsed: Some(started.elapsed()),
                next: next.clone(),
            },
        ),
        DevEnd::Failed { reason } => (
            RUN_FAILURE_STATUS,
            ResultLine {
                failed: true,
                title: "sqb dev stopped".to_owned(),
                detail: reason.clone(),
                elapsed: None,
                next: Vec::new(),
            },
        ),
    }
}

/// How long the session ran, as `42m 10s`.
fn ran(started: Instant) -> String {
    let seconds: u64 = started.elapsed().as_secs();
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m {:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h {:02}m", seconds / 3600, (seconds % 3600) / 60),
    }
}
