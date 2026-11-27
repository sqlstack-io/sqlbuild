/// Command-line arguments and where the launch record for them is looked up.
pub(crate) struct LaunchRecordPathTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) expected_project: Option<&'static str>,
}

/// One invocation keyed beside `sqb compile` in `/work`.
pub(crate) struct LaunchKeyTestCase {
    pub(crate) description: &'static str,
    pub(crate) arguments: &'static [&'static str],
    pub(crate) cwd: &'static str,
    pub(crate) expected_same: bool,
}

/// The purge delay the launcher inherits and the default it adds for the Python CLI.
pub(crate) struct AllocatorDefaultTestCase {
    pub(crate) description: &'static str,
    pub(crate) inherited: Option<&'static str>,
    pub(crate) expected_default: Option<(&'static str, &'static str)>,
}
