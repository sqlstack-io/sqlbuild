//! The variables this process adds to, or removes from, the environment it was started with.

use std::ffi::OsString;

/// Changes to the inherited environment that in-process `environment_*` reads and every started child see.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnvironmentOverlay {
    /// Variables set, or replaced, in order.
    pub set: Vec<(OsString, OsString)>,
    /// Inherited variables that are unset.
    pub remove: Vec<OsString>,
}

/// Which env file a process tree loaded, recorded in `SQLBUILD_ENV_FILE_LOADED`; never any value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnvFileMarker {
    /// The project directory the file was chosen for.
    pub project_dir: String,
    /// The target whose `env_file` was used, `None` without a selected target.
    pub target: Option<String>,
    /// The resolved path, `None` when `env_file = ""` turns loading off.
    pub path: Option<String>,
    /// Whether the file existed and was read.
    pub loaded: bool,
    /// The variables taken from the file, sorted; the process environment kept the rest.
    pub names: Vec<String>,
    /// Variables the file sets that the process environment already set, sorted.
    pub overridden: Vec<String>,
}
