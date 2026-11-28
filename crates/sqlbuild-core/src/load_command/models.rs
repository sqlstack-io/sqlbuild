//! What a project can load, as `sqb load` reads it.

/// The kinds of loading node a project has; a build selector naming a kind the project lacks is an error.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LoadKinds {
    /// A sources file declares a `load:`.
    pub loads: bool,
    /// A Python file declares a `@loader`.
    pub loaders: bool,
    /// A sources file lists a source.
    pub sources: bool,
}
