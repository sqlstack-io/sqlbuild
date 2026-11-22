//! Where a missing secret was looked for, as messages name it.

use std::path::Path;

use crate::discovery::main::sources::env_file_display::env_file_display;

/// `the environment and from .env`, or `the environment` when the project loads no env file.
pub fn secret_places(project_dir: &Path) -> String {
    match env_file_display(project_dir) {
        Some(shown) => format!("the environment and from {shown}"),
        None => "the environment".to_owned(),
    }
}
