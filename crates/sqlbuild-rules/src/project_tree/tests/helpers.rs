use std::os::unix::fs::symlink;
use std::path::PathBuf;

/// A project with nested, hidden, tooling and linked paths, and its resolved root.
pub(crate) fn linked_project() -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().expect("temporary project");
    let root: PathBuf = std::fs::canonicalize(directory.path()).expect("resolved root");
    for folder in ["sql/models/marts", "sql/models/.hidden", "logs", "sub/logs"] {
        std::fs::create_dir_all(root.join(folder)).expect("folder");
    }
    for file in [
        "sql/models/marts/orders.sql",
        "sql/models/.dot.sql",
        "sql/models/.hidden/x.sql",
    ] {
        std::fs::write(root.join(file), "select 1\n").expect("file");
    }
    std::fs::write(root.join("readme.md"), "# r\n").expect("readme");
    std::fs::write(root.join("sub/logs/kept.sql"), "k").expect("kept");
    symlink("marts/orders.sql", root.join("sql/models/link.sql")).expect("link");
    symlink("marts", root.join("sql/models/linkdir")).expect("dir link");
    symlink("../readme.md", root.join("sql/models/readme.sql")).expect("readme link");
    (directory, root)
}
