//! What one imported `python/nodes/` module declares.

/// The nodes one module declares directly and through its factories.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeModule {
    /// The project-relative path, `/`-separated.
    pub relative_path: String,
    /// Each decorated node's name and kind (`loader`, `asset` or `check`).
    pub direct: Vec<(String, &'static str)>,
    /// Each factory's name and the kinds of the nodes it returned, without repeats.
    pub factories: Vec<(String, Vec<&'static str>)>,
}

/// The files one kind of node is declared in, and its plural as messages name it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct KindFiles {
    pub(crate) kind: &'static str,
    /// The file of one direct declaration.
    pub(crate) direct: &'static str,
    /// The file of a factory that makes only this kind.
    pub(crate) factory: &'static str,
    pub(crate) plural: &'static str,
}
