//! The file each Python node declaration belongs in.

/// The file of one direct `@loader`, `@asset` or `@check`, by kind.
pub(crate) const DIRECT_FILES: [(&str, &str); 3] =
    [("loader", "loader.py"), ("asset", "asset.py"), ("check", "checks.py")];
/// The file of a factory that makes nodes of one kind, by kind.
pub(crate) const SINGLE_KIND_FACTORY_FILES: [(&str, &str); 3] = [
    ("loader", "loaders.py"),
    ("asset", "assets.py"),
    ("check", "checks.py"),
];
/// The file of a factory that makes nodes of more than one kind.
pub(crate) const MIXED_FACTORY_FILE: &str = "factory.py";
/// The plural of each kind, as messages name what a factory makes.
pub(crate) const KIND_PLURALS: [(&str, &str); 3] =
    [("loader", "loaders"), ("asset", "assets"), ("check", "checks")];
/// Declaration files that hold exactly one node.
pub(crate) const SINGLE_NODE_FILES: [&str; 2] = ["loader.py", "asset.py"];
/// The file of a unit's checks, which may sit beside its node declaration file.
pub(crate) const CHECKS_FILE: &str = "checks.py";
