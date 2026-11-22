//! Open the dev target's first connection in the foreground: the client owns a pseudo-terminal the screen types into as its controlling terminal, so Ctrl+C stays with `sqb dev` and a password prompt can turn echo off itself.

use std::fs::File;
use std::io::Read;
use std::process::Stdio;

use sqlbuild_core::screen_text::models::WarehouseState;
use sqlbuild_orchestration::dev_events::constants::{DEBUG_REPORT_VARIABLE, DEV_EVENTS_VARIABLE};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc::UnboundedSender;

use crate::dev_screen::models::{DevStep, KeyInput, ScreenMessage};
use crate::dev_session::_helpers::connection::{connection_outcome, split_debug_output};
use crate::dev_session::constants::{CONNECTING, DEBUG_DRAIN, NO_REPORT};
use crate::dev_session::models::DevProject;
use crate::dev_stop::main::stop_switch::stop_switch;
use sqlbuild_core::process_environment::main::apply_environment_overlay::apply_environment_overlay;

/// Run `sqb debug --json`, showing the client's own text as a prompt above the live steps; the connection's state.
pub(crate) async fn connect_in_foreground(
    screen: &UnboundedSender<ScreenMessage>,
    keys: Option<&KeyInput>,
    dev: &DevProject,
) -> WarehouseState {
    drop(screen.send(ScreenMessage::Started {
        step: DevStep::Warehouse,
        detail: CONNECTING.to_owned(),
    }));
    let (adapter, state): (String, WarehouseState) = match run_debug(screen, keys, dev).await {
        Ok(report) => connection_outcome(&report),
        Err(error) => (
            String::new(),
            WarehouseState::Failed {
                message: format!("{NO_REPORT}: {error}"),
            },
        ),
    };
    drop(screen.send(ScreenMessage::Warehouse {
        adapter,
        state: state.clone(),
    }));
    state
}

/// While the client runs: typed keys go to its terminal, and its process group dies with `sqb dev`.
struct Foreground<'a> {
    keys: Option<&'a KeyInput>,
    group: Option<u32>,
}

impl Drop for Foreground<'_> {
    fn drop(&mut self) {
        if let Some(keys) = self.keys
            && let Ok(mut forward) = keys.forward.lock()
        {
            *forward = None;
        }
        if let Some(group) = self.group {
            stop_switch().forget_group(group);
        }
    }
}

/// The report from its own file; the client's text on stdout and stderr streamed to the screen, until `sqb debug` exits even if a browser it opened keeps the pipes.
async fn run_debug(
    screen: &UnboundedSender<ScreenMessage>,
    keys: Option<&KeyInput>,
    dev: &DevProject,
) -> std::io::Result<Vec<u8>> {
    let report: ReportFile = ReportFile(
        std::env::temp_dir().join(format!("sqb-dev-debug-{}.json", uuid::Uuid::now_v7())),
    );
    let mut command = Command::new(&dev.executable);
    apply_environment_overlay(command.as_std_mut());
    command
        .arg("--project-dir")
        .arg(&dev.project)
        .args(["debug", "--target", &dev.target, "--json"])
        .env_remove(DEV_EVENTS_VARIABLE)
        .env(DEBUG_REPORT_VARIABLE, &report.0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let terminal: Option<(File, File)> = match keys.map(|_| open_terminal()) {
        Some(Ok(terminal)) => Some(terminal),
        Some(Err(error)) => {
            drop(screen.send(ScreenMessage::Log(format!(
                "the warehouse client reads this terminal directly: {error}"
            ))));
            None
        }
        None => None,
    };
    match &terminal {
        Some((_, client)) => {
            command.stdin(Stdio::from(client.try_clone()?));
            #[cfg(unix)]
            sqlbuild_process_ownership::terminal::main::controlling_terminal::controlling_terminal(
                command.as_std_mut(),
            );
        }
        None => {
            command.stdin(Stdio::inherit());
        }
    }
    let mut child = command.spawn()?;
    let mut foreground: Foreground<'_> = Foreground { keys, group: None };
    if let Some((controller, client)) = terminal {
        drop(client);
        if let Some(pid) = child.id() {
            stop_switch().register_group(pid);
            foreground.group = Some(pid);
        }
        if let Some(keys) = keys
            && let Ok(mut forward) = keys.forward.lock()
        {
            *forward = Some(controller.try_clone()?);
        }
        echo_terminal(controller, screen.clone());
    }
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(std::io::Error::other("sqb debug output is unavailable"));
    };
    let errors = async {
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
    let output = async {
        let mut chunk: [u8; 4096] = [0; 4096];
        loop {
            let read: usize = stdout.read(&mut chunk).await?;
            if read == 0 {
                return Ok::<(), std::io::Error>(());
            }
            drop(screen.send(ScreenMessage::ClientOutput(
                String::from_utf8_lossy(&chunk[..read]).into_owned(),
            )));
        }
    };
    {
        let reading = async { tokio::try_join!(errors, output) };
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
    drop(foreground);
    std::fs::read(&report.0)
}

/// The file `sqb debug` writes its report to, removed once read or abandoned.
struct ReportFile(std::path::PathBuf);

impl Drop for ReportFile {
    fn drop(&mut self) {
        drop(std::fs::remove_file(&self.0));
    }
}

/// Anything the client writes to its terminal, shown in the prompt, with what is typed when the client leaves echo on.
fn echo_terminal(controller: File, screen: UnboundedSender<ScreenMessage>) {
    std::thread::spawn(move || {
        let mut controller: File = controller;
        let mut chunk: [u8; 1024] = [0; 1024];
        while let Ok(read) = controller.read(&mut chunk) {
            if read == 0
                || screen
                    .send(ScreenMessage::ClientOutput(
                        String::from_utf8_lossy(&chunk[..read]).into_owned(),
                    ))
                    .is_err()
            {
                return;
            }
        }
    });
}

/// A pseudo-terminal: the controller `sqb dev` writes keys to, and the terminal the client reads, sized like this one. It echoes as a terminal does, so a plain prompt shows what is typed and a password prompt that turns echo off hides it. Outside Linux `openpt` can't open close-on-exec, so that is set straight after.
#[cfg(unix)]
fn open_terminal() -> std::io::Result<(File, File)> {
    use rustix::fs::{Mode, OFlags};
    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
    #[cfg(target_os = "linux")]
    let controller = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY | OpenptFlags::CLOEXEC)?;
    #[cfg(not(target_os = "linux"))]
    let controller = {
        let controller = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
        rustix::io::fcntl_setfd(&controller, rustix::io::FdFlags::CLOEXEC)?;
        controller
    };
    grantpt(&controller)?;
    unlockpt(&controller)?;
    let name = ptsname(&controller, Vec::new())?;
    let client = rustix::fs::open(
        name.as_c_str(),
        OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    if let Ok(size) = rustix::termios::tcgetwinsize(std::io::stdin()) {
        let _ = rustix::termios::tcsetwinsize(&client, size);
    }
    Ok((File::from(controller), File::from(client)))
}

#[cfg(not(unix))]
fn open_terminal() -> std::io::Result<(File, File)> {
    Err(std::io::Error::other("pseudo-terminals need Unix"))
}
