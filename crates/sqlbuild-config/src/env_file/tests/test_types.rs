/// One project, its env file and the variables a load takes from it.
pub(super) struct EnvFileCase {
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
