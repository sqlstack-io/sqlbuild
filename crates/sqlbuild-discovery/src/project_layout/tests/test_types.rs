/// One project's entries, its extra paths, and the entries the layout check reports.
pub(super) struct LayoutTestCase {
    pub(super) description: &'static str,
    pub(super) files: &'static [&'static str],
    pub(super) folders: &'static [&'static str],
    pub(super) gitignore: &'static str,
    pub(super) extra_paths: &'static [&'static str],
    pub(super) expected_paths: &'static [&'static str],
}

/// Paths SQLBuild writes, from inside or outside a project, and the top-level entries they exempt.
pub(super) struct WrittenEntriesTestCase {
    pub(super) description: &'static str,
    pub(super) written: &'static [&'static str],
    pub(super) expected_entries: &'static [&'static str],
}
