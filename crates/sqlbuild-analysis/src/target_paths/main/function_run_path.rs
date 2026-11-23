//! Where a function's executed SQL is written under `target/run`.

use crate::target_paths::_helpers::path_text::with_suffix;
use crate::target_paths::constants::{FUNCTIONS_DIR, SQL_FILE_SUFFIX};

/// The function's path under `sql/functions/<language>/`, with a `.sql` suffix.
#[must_use]
pub fn function_run_path(relative_path: &str, language: &str) -> String {
    let root: String = format!("{FUNCTIONS_DIR}/{language}/");
    let path: String = if relative_path.starts_with(&root) {
        relative_path.to_owned()
    } else {
        format!("{root}{relative_path}")
    };
    with_suffix(&path, SQL_FILE_SUFFIX)
}
