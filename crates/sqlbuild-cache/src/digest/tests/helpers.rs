use std::collections::HashSet;
use std::path::Path;

/// The model file the fingerprint test project leaves to the cache key.
pub(crate) const EXCLUDED_MODEL: &str = "sql/models/orders.sql";

/// Write a small project: config, a macro, a model and files in skipped folders.
pub(crate) fn write_project(root: &Path) {
    for (relative_path, contents) in [
        ("sqlbuild_project.toml", "name = \"orders\"\n"),
        ("sql/macros/cents.py", "def cents(amount):\n    return amount\n"),
        (EXCLUDED_MODEL, "SELECT 1 AS id\n"),
        ("target/cache/store.bin", "old"),
        ("sql/macros/__pycache__/cents.pyc", "bytecode"),
        ("warehouse.duckdb", "pages"),
    ] {
        write_file(root, relative_path, contents);
    }
}

pub(crate) fn write_file(root: &Path, relative_path: &str, contents: &str) {
    let path = root.join(relative_path);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent folder");
    std::fs::write(path, contents).expect("project file");
}

pub(crate) fn excluded_files() -> HashSet<String> {
    HashSet::from([EXCLUDED_MODEL.to_owned()])
}

/// The modification time Python's oracle gave the file at `index` in sorted path order.
const BASE_MODIFIED_NANOSECONDS: u64 = 1_700_000_000_000_000_000;
const MODIFIED_STEP_NANOSECONDS: u64 = 1_234_567;

/// Write `files` below `root` with the oracle's modification times and a symlinked folder.
pub(crate) fn write_package(root: &Path, files: &[(&str, &str)]) {
    let mut sorted: Vec<&(&str, &str)> = files.iter().collect();
    sorted.sort_by_key(|(path, _)| *path);
    for (index, (relative_path, contents)) in sorted.into_iter().enumerate() {
        write_file(root, relative_path, contents);
        let nanoseconds: u64 = BASE_MODIFIED_NANOSECONDS
            + u64::try_from(index).expect("few files") * MODIFIED_STEP_NANOSECONDS;
        let modified = std::time::UNIX_EPOCH + std::time::Duration::from_nanos(nanoseconds);
        std::fs::File::options()
            .write(true)
            .open(root.join(relative_path))
            .and_then(|file| file.set_modified(modified))
            .expect("the modification time is set");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink("pkg", root.join("link_dir")).expect("the folder link is made");
}
