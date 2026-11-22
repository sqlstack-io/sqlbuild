//! Keep the project env file out of git.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;

use crate::init_command::constants::{ENV_FILE_IGNORE, GITIGNORE_FILE};

/// Add `.env` to `directory/.gitignore`, creating it, unless a line already ignores it.
pub fn ignore_env_file(directory: &Path) -> std::io::Result<()> {
    let path = directory.join(GITIGNORE_FILE);
    let existing: String = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };
    if existing
        .lines()
        .any(|line| matches!(line.trim(), ".env" | "/.env"))
    {
        return Ok(());
    }
    let separator: &str = if existing.is_empty() || existing.ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(format!("{separator}{ENV_FILE_IGNORE}").as_bytes())
}
