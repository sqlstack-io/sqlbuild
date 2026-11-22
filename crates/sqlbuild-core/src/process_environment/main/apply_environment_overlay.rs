//! The environment overlay on a command that starts a child process.

use std::process::Command;

use crate::process_environment::_helpers::overlay::current;

/// Give a child the env file variables this process loaded, and unset the ones it dropped.
pub fn apply_environment_overlay(command: &mut Command) {
    let overlay = current();
    for name in overlay.remove {
        command.env_remove(name);
    }
    command.envs(overlay.set);
}
