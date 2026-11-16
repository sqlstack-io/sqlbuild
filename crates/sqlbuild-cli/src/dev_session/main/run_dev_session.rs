//! Feed the `sqb dev` screen from the location child, open the warehouse in the foreground and act on hotkeys.

use std::io::IsTerminal;

use sqlbuild_orchestration::declarations::main::list_triggers::list_triggers;
use sqlbuild_orchestration::declarations::models::{TriggerRecord, TriggerStatus};
use sqlbuild_orchestration::dev_events::main::dev_control_line::dev_control_line;
use sqlbuild_orchestration::dev_events::main::parse_dev_event::parse_dev_event;
use sqlbuild_orchestration::dev_events::models::{DevControl, DevEvent};
use sqlbuild_orchestration::runs::main::list_runs::list_runs;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch;
use uuid::Uuid;

use crate::argv::models::Stream;
use crate::dev_screen::models::{ScreenAction, ScreenMessage};
use crate::dev_screen::types::DevScreenParts;
use crate::dev_session::_helpers::foreground::connect_in_foreground;
use crate::dev_session::_helpers::runs::{SeenRuns, run_messages};
use crate::dev_session::constants::{RUN_PAGE, RUN_POLL};
use crate::dev_session::models::{DevProject, LocationPipes};
use crate::native_commands::_helpers::terminal::{open_browser, write_output};

/// Run until `q`, shutdown or the location child closing its output.
pub(crate) async fn run_dev_session(
    (screen, mut actions, keys): DevScreenParts,
    (pipes, dev): (LocationPipes, DevProject),
    (store, location_id, url): (ControlStore, Uuid, String),
    mut shutdown: watch::Receiver<bool>,
) -> Result<(), ControlError> {
    let LocationPipes {
        mut stdin,
        stdout,
        stderr,
    } = pipes;
    tokio::spawn(relay_logs(BufReader::new(stderr), screen.clone()));
    let mut events = BufReader::new(stdout).lines();
    let mut runs: Option<SeenRuns> = None;
    let mut connected: Option<bool> = None;
    let mut poll = tokio::time::interval(RUN_POLL);
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return Ok(());
                }
            }
            line = events.next_line() => {
                let Some(line) = line? else {
                    return Ok(());
                };
                let Some(event) = parse_dev_event(&line) else {
                    drop(screen.send(ScreenMessage::Log(line)));
                    continue;
                };
                if let DevEvent::Published { record, .. } = &event {
                    publish_record(record);
                    drop(screen.send(ScreenMessage::Triggers(
                        trigger_summary(&list_triggers(&store, location_id, &dev.target).await?),
                    )));
                }
                let first: bool = matches!(event, DevEvent::Published { .. }) && connected.is_none();
                drop(screen.send(ScreenMessage::Child(event)));
                if first {
                    let verdict: bool = connect_in_foreground(&screen, keys.as_deref(), &dev).await;
                    connected = Some(verdict);
                    send_control(&mut stdin, DevControl::Warehouse { connected: verdict }).await?;
                }
            }
            action = actions.recv() => match action {
                Some(ScreenAction::Quit) | None => return Ok(()),
                Some(ScreenAction::OpenUi) => drop(open_browser(&url)),
                Some(ScreenAction::Recompile) => send_control(&mut stdin, DevControl::Recompile).await?,
                Some(ScreenAction::RetryWarehouse) => {
                    let verdict: bool = connect_in_foreground(&screen, keys.as_deref(), &dev).await;
                    connected = Some(verdict);
                    send_control(&mut stdin, DevControl::Warehouse { connected: verdict }).await?;
                }
                Some(ScreenAction::None) => {}
            },
            _ = poll.tick(), if connected.is_some() => {
                let records = list_runs(&store, (location_id, &dev.target), RUN_PAGE).await?;
                let messages;
                (messages, runs) = {
                    let (messages, seen) = run_messages(runs, &records);
                    (messages, Some(seen))
                };
                for message in messages {
                    drop(screen.send(message));
                }
            }
        }
    }
}

/// The child's log lines, kept for `l`.
async fn relay_logs(
    stderr: BufReader<tokio::process::ChildStderr>,
    screen: UnboundedSender<ScreenMessage>,
) {
    let mut lines = stderr.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        drop(screen.send(ScreenMessage::Log(line)));
    }
}

/// Redirected stdout keeps one publication record per line, as `sqb location serve` prints them.
fn publish_record(record: &serde_json::Value) {
    if std::io::stdout().is_terminal() {
        return;
    }
    let mut line: String = record.to_string();
    line.push('\n');
    drop(write_output(&[(Stream::Stdout, &line)]));
}

async fn send_control(stdin: &mut ChildStdin, control: DevControl) -> Result<(), ControlError> {
    stdin
        .write_all(dev_control_line(control).as_bytes())
        .await?;
    stdin.flush().await?;
    Ok(())
}

/// `41 paused`, `3 running · 38 paused` or `none`.
fn trigger_summary(triggers: &[TriggerRecord]) -> String {
    let paused: usize = triggers
        .iter()
        .filter(|trigger| trigger.status == TriggerStatus::Paused)
        .count();
    let running: usize = triggers.len() - paused;
    match (running, paused) {
        (0, 0) => "none".to_owned(),
        (0, paused) => format!("{paused} paused"),
        (running, 0) => format!("{running} running"),
        (running, paused) => format!("{running} running \u{b7} {paused} paused"),
    }
}
