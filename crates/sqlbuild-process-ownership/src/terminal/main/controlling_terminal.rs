//! Start a child in its own session with the terminal on its stdin as its controlling terminal.

use std::process::Command;

use crate::terminal::_helpers::session::own_session_terminal;

/// Before `command`'s program runs: a new session, so its process group can be signalled as one, whose controlling terminal is the terminal on its stdin, so `/dev/tty` prompts such as `getpass` reach it.
pub fn controlling_terminal(command: &mut Command) -> &mut Command {
    own_session_terminal(command)
}
