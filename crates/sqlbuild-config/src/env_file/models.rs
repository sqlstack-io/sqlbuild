//! Which env file a command loads, what it took from it, and why it could not.

use std::path::PathBuf;

use sqlbuild_core::process_environment::models::{EnvFileMarker, EnvironmentOverlay};

/// The env file the project and target choose, before it is read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvFileChoice {
    /// The resolved path; `None` when `env_file = ""` turns loading off.
    pub path: Option<PathBuf>,
    /// The path as written, such as `.env`, for messages.
    pub display: String,
    /// Whether `env_file` was set; a missing file is then an error rather than skipped.
    pub configured: bool,
    /// Where it was set, such as `[targets.uat] env_file in sqlbuild_project.toml`.
    pub setting: String,
}

/// What loading the chosen env file gives every process this command starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnvFileLoad {
    pub choice: EnvFileChoice,
    pub overlay: EnvironmentOverlay,
    pub marker: EnvFileMarker,
    /// Set when the file is readable by its group or others.
    pub warning: Option<String>,
}

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

/// Where the chosen path came from, for `EnvFileChoice::setting`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChoiceSource {
    pub(crate) text: Option<String>,
    pub(crate) target: Option<String>,
    pub(crate) project_dir: PathBuf,
}
