/// One project, its env file and the variables a load takes from it.
pub(super) struct EnvFileTestCase {
    pub(super) description: &'static str,
    /// `sqlbuild_project.toml` after the required `name` and `adapter`.
    pub(super) project: &'static str,
    /// Files to write, relative to the project directory.
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) target: Option<&'static str>,
    pub(super) expected_display: &'static str,
    pub(super) expected_loaded: bool,
    /// `(name, value)` set for every process, the marker aside.
    pub(super) expected_set: &'static [(&'static str, &'static str)],
}

/// A `.env` some of whose variables the environment already sets.
pub(super) struct EnvironmentWinsTestCase {
    pub(super) description: &'static str,
    pub(super) file: &'static str,
    pub(super) expected_set: &'static [(&'static str, &'static str)],
    pub(super) expected_names: &'static [&'static str],
    pub(super) expected_overridden: &'static [&'static str],
}

/// A project whose env file cannot load, and the error's message.
pub(super) struct EnvFileFailureTestCase {
    pub(super) description: &'static str,
    pub(super) project: &'static str,
    pub(super) files: &'static [(&'static str, &'static str)],
    pub(super) target: Option<&'static str>,
    pub(super) expected_message: &'static str,
}

/// A `.env` with permission bits, and the warning a load gives.
pub(super) struct SharedReadTestCase {
    pub(super) description: &'static str,
    pub(super) mode: u32,
    pub(super) expected_warning: Option<&'static str>,
}

/// A `.env` loaded again after an earlier load took `earlier` from it.
pub(super) struct ReloadTestCase {
    pub(super) description: &'static str,
    pub(super) file: &'static str,
    pub(super) earlier: &'static [(&'static str, &'static str)],
    pub(super) expected_set: &'static [(&'static str, &'static str)],
    pub(super) expected_remove: &'static [&'static str],
}
