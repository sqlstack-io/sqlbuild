//! The location child's pipes: its log lines and the control lines sent to it.

use sqlbuild_orchestration::dev_events::main::dev_control_line::dev_control_line;
use sqlbuild_orchestration::dev_events::models::DevControl;
use sqlbuild_orchestration::store::errors::ControlError;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::mpsc::UnboundedSender;

use crate::dev_screen::models::ScreenMessage;

/// The child's log lines, kept for `l`.
pub(crate) async fn relay_logs(
    stderr: BufReader<tokio::process::ChildStderr>,
    screen: UnboundedSender<ScreenMessage>,
) {
    let mut lines = stderr.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        drop(screen.send(ScreenMessage::Log(line)));
    }
}

/// Send one control line to the location child.
pub(crate) async fn send_control(
    stdin: &mut ChildStdin,
    control: DevControl,
) -> Result<(), ControlError> {
    stdin
        .write_all(dev_control_line(control).as_bytes())
        .await?;
    stdin.flush().await?;
    Ok(())
}
