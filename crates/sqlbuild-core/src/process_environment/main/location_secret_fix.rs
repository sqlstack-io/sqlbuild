//! How to set a missing secret for a location, whose environment only a restart changes.

/// Where to set `subject` and what then picks it up: `r` re-reads an env file under `sqb dev`, and only a restart reads a changed environment.
#[must_use]
pub fn location_secret_fix(subject: &str, env_file: Option<&str>) -> String {
    let environment: &str =
        "in the environment the location (or sqb dev) was started with, then restart it";
    match env_file {
        Some(file) => format!(
            "set {subject} in {file}, then restart the location (in sqb dev, press r), or {environment}"
        ),
        None => format!("set {subject} {environment}"),
    }
}
