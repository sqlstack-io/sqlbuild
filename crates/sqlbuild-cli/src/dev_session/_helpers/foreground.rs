//! Open the dev target's first connection in the foreground, attached to the terminal.

use std::process::Stdio;

use sqlbuild_core::screen_text::models::WarehouseState;
use sqlbuild_orchestration::dev_events::constants::DEV_EVENTS_VARIABLE;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use crate::dev_screen::models::{DevStep, KeyGate, ScreenMessage};
use crate::dev_session::_helpers::connection::{connection_outcome, split_debug_output};
use crate::dev_session::constants::{CONNECTING, DEBUG_DRAIN, NO_REPORT};
use crate::dev_session::models::DevProject;
use crate::native_commands::_helpers::command::environment::with_environment_overlay;

/// Run `sqb debug --json` with the terminal's input, framing the client's own output; true when it connected.
pub(crate) async fn connect_in_foreground(
    screen: &UnboundedSender<ScreenMessage>,
    keys: Option<&KeyGate>,
    dev: &DevProject,
) -> bool {
    drop(screen.send(ScreenMessage::Started {
        step: DevStep::Warehouse,
        detail: CONNECTING.to_owned(),
    }));
    if let Some(keys) = keys {
        keys.close().await;
    }
    let (adapter, state): (String, WarehouseState) = match run_debug(screen, dev).await {
        Ok(report) => connection_outcome(&report),
        Err(error) => (
            String::new(),
            WarehouseState::Failed {
                message: format!("{NO_REPORT}: {error}"),
            },
        ),
    };
    if let Some(keys) = keys {
        keys.open();
    }
    let connected: bool = matches!(state, WarehouseState::Connected { .. });
    drop(screen.send(ScreenMessage::Warehouse { adapter, state }));
    connected
}

/// The report on stdout and stderr streamed to the screen, until `sqb debug` exits even if a browser it opened keeps the pipes.
async fn run_debug(
    screen: &UnboundedSender<ScreenMessage>,
    dev: &DevProject,
) -> std::io::Result<Vec<u8>> {
    let mut command = Command::new(&dev.executable);
    with_environment_overlay(&mut command);
    let mut child = command
        .arg("--project-dir")
        .arg(&dev.project)
        .args(["debug", "--target", &dev.target, "--json"])
        .env_remove(DEV_EVENTS_VARIABLE)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(std::io::Error::other("sqb debug output is unavailable"));
    };
    let mut report: Vec<u8> = Vec::new();
    let relay = async {
        let mut chunk: [u8; 4096] = [0; 4096];
        loop {
            let read: usize = stderr.read(&mut chunk).await?;
            if read == 0 {
                return Ok::<(), std::io::Error>(());
            }
            let (client, rows) = split_debug_output(&String::from_utf8_lossy(&chunk[..read]));
            if !client.is_empty() {
                drop(screen.send(ScreenMessage::ClientOutput(client)));
            }
            for row in rows {
                drop(screen.send(ScreenMessage::Log(row)));
            }
        }
    };
    {
        let reading = async { tokio::try_join!(relay, stdout.read_to_end(&mut report)) };
        tokio::pin!(reading);
        tokio::select! {
            read = &mut reading => { read?; }
            exited = child.wait() => {
                exited?;
                if let Ok(read) = tokio::time::timeout(DEBUG_DRAIN, &mut reading).await {
                    read?;
                }
            }
        }
    }
    child.wait().await?;
    Ok(report)
}
