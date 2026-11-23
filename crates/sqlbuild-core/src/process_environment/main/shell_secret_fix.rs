//! How to set a missing secret for a command run from a shell.

/// `set <subject> in this shell or in .env`, naming the env file the project loads.
#[must_use]
pub fn shell_secret_fix(subject: &str, env_file: Option<&str>) -> String {
    match env_file {
        Some(file) => format!("set {subject} in this shell or in {file}"),
        None => format!("set {subject} in this shell"),
    }
}
