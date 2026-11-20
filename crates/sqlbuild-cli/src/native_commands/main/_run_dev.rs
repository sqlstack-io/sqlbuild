//! Collapse the production API, scheduler and outbound location into one local process tree.

use std::path::PathBuf;
use std::time::Instant;

use sqlbuild_core::screen_text::models::ResultLine;

use crate::argv::models::{Namespace, Stream};
use crate::dev_screen::main::start_dev_screen::start_dev_screen;
use crate::dev_screen::models::{DevScreenHandle, DevStep, ScreenEnd, ScreenMessage};
use crate::dev_stop::constants::{RUNTIME_SHUTDOWN, START_FAILURE_STATUS};
use crate::dev_stop::main::stop_switch::stop_switch;
use crate::dev_stop::main::watch_signals::watch_signals;
use crate::dev_stop::models::DevEnd;
use crate::native_commands::_helpers::command::decision::{flag, text};
use crate::native_commands::_helpers::command::project_facts::development_config;
use crate::native_commands::_helpers::command::terminal::write_output;
use crate::native_commands::_helpers::location::dev_start::{result_line, start_roles};
use crate::native_commands::_helpers::location::execution::screen_setup;

/// Run `sqb dev` until it is stopped or fails, leave its result line, and return its exit status.
async fn run_dev(namespace: &Namespace) -> i32 {
    let started: Instant = Instant::now();
    let project: PathBuf =
        match std::fs::canonicalize(text(namespace, "project_dir").unwrap_or_else(|| ".".into())) {
            Ok(project) => project,
            Err(error) => {
                drop(write_output(&[(
                    Stream::Stderr,
                    &format!("\u{2717} sqb dev did not start  project directory: {error}\n"),
                )]));
                return START_FAILURE_STATUS;
            }
        };
    if let Err(error) = watch_signals() {
        drop(write_output(&[(
            Stream::Stderr,
            &format!("\u{2717} sqb dev did not start  signals: {error}\n"),
        )]));
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
                problem: *problem,
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
        Ok(_) => {
            start_roles(
                namespace,
                (project, target),
                (messages.clone(), actions, keys),
            )
            .await
        }
    };
    let (status, result): (i32, ResultLine) = result_line(&end, started);
    let show_log: bool = matches!(
        &end,
        DevEnd::NotStarted {
            explained: false,
            ..
        }
    ) || matches!(&end, DevEnd::Failed { .. });
    drop(finish.send(ScreenEnd { result, show_log }));
    drop(task.await);
    stop_switch().kill_all();
    stop_switch().restore_terminal();
    status
}

/// `sqb dev`: its own result line and exit status; leftover tasks get a moment, never a hang.
pub(super) fn run_development(namespace: &Namespace) -> i32 {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            drop(write_output(&[(
                Stream::Stderr,
                &format!("\u{2717} sqb dev did not start  {error}\n"),
            )]));
            return START_FAILURE_STATUS;
        }
    };
    let status: i32 = runtime.block_on(run_dev(namespace));
    runtime.shutdown_timeout(RUNTIME_SHUTDOWN);
    status
}
