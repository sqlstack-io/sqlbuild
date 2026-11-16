//! Route this process's stderr into the screen's log while the screen owns the terminal.

use std::fs::File;
#[cfg(unix)]
use std::io::{BufRead, BufReader};
#[cfg(unix)]
use std::os::fd::AsFd;

use tokio::sync::mpsc::UnboundedSender;

use crate::dev_screen::models::ScreenMessage;

/// Point stderr at a pipe whose lines become log messages, and return the terminal it pointed at.
#[cfg(unix)]
pub(crate) fn capture_stderr(
    messages: UnboundedSender<ScreenMessage>,
) -> std::io::Result<Option<File>> {
    let terminal: File = File::from(std::io::stderr().as_fd().try_clone_to_owned()?);
    let (reader, writer) = std::io::pipe()?;
    rustix::stdio::dup2_stderr(&writer)?;
    drop(writer);
    std::thread::spawn(move || {
        for line in BufReader::new(reader).split(b'\n') {
            let Ok(line) = line else {
                return;
            };
            let text: String = String::from_utf8_lossy(&line)
                .trim_end_matches('\r')
                .to_owned();
            if messages.send(ScreenMessage::Log(text)).is_err() {
                return;
            }
        }
    });
    Ok(Some(terminal))
}

/// Without file-descriptor redirection, stderr stays on the terminal.
#[cfg(not(unix))]
pub(crate) fn capture_stderr(
    _messages: UnboundedSender<ScreenMessage>,
) -> std::io::Result<Option<File>> {
    Ok(None)
}

/// Point stderr back at the terminal.
#[cfg(unix)]
pub(crate) fn release_stderr(terminal: &File) {
    let _ = rustix::stdio::dup2_stderr(terminal);
}

/// Nothing was redirected.
#[cfg(not(unix))]
pub(crate) fn release_stderr(_terminal: &File) {}
