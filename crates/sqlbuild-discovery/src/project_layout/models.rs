//! One project entry outside the expected layout.

/// An entry the project layout does not allow, with what its folder may hold instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayoutProblem {
    /// The project-relative path, `/`-separated, with a trailing `/` for a folder.
    pub path: String,
    /// The folder holding the entry, or `None` at the project's top level.
    pub folder: Option<&'static str>,
    /// What that folder may hold, each folder with a trailing `/`.
    pub allowed: Vec<String>,
}

/// One folder the check reads and the entries it may hold.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Checked<'a> {
    pub(crate) folder: Option<&'static str>,
    pub(crate) directories: &'a [&'a str],
    pub(crate) files: &'a [&'a str],
}
