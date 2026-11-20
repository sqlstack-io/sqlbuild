//! The environment overlay on the commands that start `sqb` children.

use sqlbuild_core::process_environment::main::environment_overlay::environment_overlay;

/// Give a child the env file variables this process loaded, and unset the ones it dropped.
pub(crate) fn with_environment_overlay(command: &mut tokio::process::Command) {
    let overlay = environment_overlay();
    for name in overlay.remove {
        command.env_remove(name);
    }
    command.envs(overlay.set);
}
