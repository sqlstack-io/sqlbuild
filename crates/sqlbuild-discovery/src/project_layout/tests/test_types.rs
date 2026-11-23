/// One project's entries, its extra paths, and the entries the layout check reports.
pub(super) struct LayoutTestCase {
    pub(super) description: &'static str,
    pub(super) files: &'static [&'static str],
    pub(super) folders: &'static [&'static str],
    pub(super) gitignore: Option<&'static str>,
    pub(super) extra_paths: &'static [&'static str],
    pub(super) expected: &'static [&'static str],
}
