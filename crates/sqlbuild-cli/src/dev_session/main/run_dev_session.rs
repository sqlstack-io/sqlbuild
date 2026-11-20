//! Feed the `sqb dev` screen from the location child, open the warehouse in the foreground and act on keys.

use std::time::Duration;

use sqlbuild_core::screen_text::models::WarehouseState;
use sqlbuild_orchestration::declarations::main::list_triggers::list_triggers;
use sqlbuild_orchestration::dev_events::main::parse_dev_event::parse_dev_event;
use sqlbuild_orchestration::dev_events::models::{DevControl, DevEvent};
use sqlbuild_orchestration::runs::main::listings::list_runs::list_runs;
use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::watch;
use uuid::Uuid;

use crate::dev_screen::models::{ScreenAction, ScreenMessage};
use crate::dev_screen::types::DevScreenParts;
use crate::dev_session::_helpers::child::{publish_record, relay_logs, send_control};
use crate::dev_session::_helpers::foreground::connect_in_foreground;
use crate::dev_session::_helpers::runs::{SeenRuns, run_messages};
use crate::dev_session::_helpers::triggers::trigger_summary;
use crate::dev_session::constants::{
    CANCELLED_CONNECT, LOCATION_STOPPED, RUN_PAGE, RUN_POLL, WAREHOUSE_FAILED,
};
use crate::dev_session::models::{DevProject, LocationPipes};
use crate::dev_session::types::Connecting;
use crate::dev_stop::main::stop_switch::stop_switch;
use crate::person_login::main::open_browser::open_browser;

/// Run until shutdown; the location child closing its output, or the first connection failing, is an error.
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
    let mut connecting: Option<Connecting<'_>> = None;
    let mut poll = tokio::time::interval(RUN_POLL);
    let connect =
        || -> Connecting<'_> { Box::pin(connect_in_foreground(&screen, keys.as_deref(), &dev)) };
    loop {
        tokio::select! {
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    if connecting.take().is_some() {
                        drop(screen.send(ScreenMessage::Stopped {
                            label: CANCELLED_CONNECT.to_owned(),
                            elapsed: Duration::ZERO,
                        }));
                    }
                    return Ok(());
                }
            }
            line = events.next_line() => {
                let Some(line) = line? else {
                    if *shutdown.borrow() {
                        return Ok(());
                    }
                    return Err(ControlError::Invalid(LOCATION_STOPPED.into()));
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
                let first: bool = matches!(event, DevEvent::Published { .. })
                    && connected.is_none()
                    && connecting.is_none();
                drop(screen.send(ScreenMessage::Child(event)));
                if first {
                    connecting = Some(connect());
                }
            }
            state = settle(&mut connecting), if connecting.is_some() => {
                connecting = None;
                let verdict: bool = matches!(state, WarehouseState::Connected { .. });
                if !verdict && connected.is_none() {
                    return Err(ControlError::Invalid(WAREHOUSE_FAILED.into()));
                }
                connected = Some(verdict);
                stop_switch().mark_ready();
                send_control(&mut stdin, DevControl::Warehouse { connected: verdict }).await?;
            }
            Some(action) = actions.recv() => match action {
                ScreenAction::OpenUi => drop(open_browser(&url)),
                ScreenAction::Recompile => send_control(&mut stdin, DevControl::Recompile).await?,
                ScreenAction::RetryWarehouse => {
                    if connecting.is_none() {
                        connecting = Some(connect());
                    }
                }
                ScreenAction::None => {}
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

/// The open connection's state once it settles; without one, never.
async fn settle(connecting: &mut Option<Connecting<'_>>) -> WarehouseState {
    match connecting {
        Some(future) => future.await,
        None => std::future::pending().await,
    }
}
