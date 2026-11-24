//! The file each Python node declaration belongs in.

use crate::node_files::models::KindFiles;

/// Where each kind of node is declared, directly or by a factory of that kind only.
pub(crate) const KIND_FILES: [KindFiles; 3] = [
    KindFiles {
        kind: "loader",
        direct: "loader.py",
        factory: "loaders.py",
        plural: "loaders",
    },
    KindFiles {
        kind: "asset",
        direct: "asset.py",
        factory: "assets.py",
        plural: "assets",
    },
    KindFiles {
        kind: "check",
        direct: "checks.py",
        factory: "checks.py",
        plural: "checks",
    },
];
/// The file of a factory that makes nodes of more than one kind.
pub(crate) const MIXED_FACTORY_FILE: &str = "factory.py";
/// Declaration files that hold exactly one node.
pub(crate) const SINGLE_NODE_FILES: [&str; 2] = ["loader.py", "asset.py"];
/// The file of a unit's checks, which may sit beside its node declaration file.
pub(crate) const CHECKS_FILE: &str = "checks.py";
