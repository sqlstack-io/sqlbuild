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

/// Where the chosen path came from, for `EnvFileChoice::setting`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ChoiceSource {
    pub(crate) text: Option<String>,
    pub(crate) target: Option<String>,
    pub(crate) project_dir: PathBuf,
}
