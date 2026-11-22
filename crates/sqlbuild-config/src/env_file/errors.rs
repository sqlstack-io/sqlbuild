//! Why the chosen env file could not be loaded.

use crate::env_file::models::EnvFileChoice;

/// Why the chosen env file could not be loaded; no message ever includes a value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnvFileError {
    /// `env_file` names a file that does not exist.
    Missing { choice: EnvFileChoice },
    /// The file exists but could not be read.
    Unreadable {
        choice: EnvFileChoice,
        reason: String,
    },
    /// A line is not `KEY=value`; only its number is reported, never its text.
    Invalid { choice: EnvFileChoice, line: usize },
}

impl EnvFileError {
    /// The one-line message, naming the file and where it was set.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::Missing { choice } => format!(
                "env file {} does not exist (set by {})",
                choice.display, choice.setting
            ),
            Self::Unreadable { choice, reason } => format!(
                "cannot read env file {} (set by {}): {reason}",
                choice.display, choice.setting
            ),
            Self::Invalid { choice, line } => format!(
                "env file {} line {line} is not KEY=value (set by {})",
                choice.display, choice.setting
            ),
        }
    }

    /// What to change.
    #[must_use]
    pub fn help(&self) -> String {
        match self {
            Self::Missing { choice } => format!(
                "create {}, or change env_file in sqlbuild_project.toml (\"\" turns it off)",
                choice.display
            ),
            Self::Unreadable { choice, .. } => {
                format!("check that {} is a readable file", choice.display)
            }
            Self::Invalid { .. } => "write each line as KEY=value, quoting values with spaces; \
                                     # starts a comment"
                .to_owned(),
        }
    }
}
