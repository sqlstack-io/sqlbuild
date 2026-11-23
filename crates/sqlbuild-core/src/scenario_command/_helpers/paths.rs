//! Path comparisons with `pathlib.PurePath` semantics, and runtime artifact paths.

use std::path::{Component, Path, PathBuf};

use crate::scenario_command::constants::{FUNCTIONS_DIR, MODELS_DIR, SQL_EXTENSION};

/// `Path(text)`: the path without `.` components or repeated separators.
pub(crate) fn normalized(path: &Path) -> PathBuf {
    path.components()
        .filter(|component| !matches!(component, Component::CurDir))
        .collect()
}

/// `path == prefix or prefix in path.parents`.
pub(crate) fn is_under(path: &Path, prefix: &Path) -> bool {
    if prefix.as_os_str().is_empty() {
        return !path.has_root();
    }
    path.starts_with(prefix)
}

/// `Path(selector).suffix == ".sql"`.
pub(crate) fn has_sql_suffix(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == SQL_EXTENSION)
}

/// A logical name safe as one file stem: path separators become `__`.
pub(crate) fn safe_file_stem(name: &str) -> String {
    name.replace(['/', '\\'], "__")
}

/// A model's artifact path, always under `sql/models/`.
pub(crate) fn model_output_path(relative_path: &Path) -> PathBuf {
    let path: PathBuf = normalized(relative_path);
    if path.starts_with(MODELS_DIR) {
        return path;
    }
    Path::new(MODELS_DIR).join(path)
}

/// A function's artifact path under `sql/functions/<language>/`, with a `.sql` suffix.
pub(crate) fn function_output_path(relative_path: &Path, language: &str) -> PathBuf {
    let path: PathBuf = normalized(relative_path);
    let under_language: bool = path.starts_with(Path::new(FUNCTIONS_DIR).join(language));
    let mut output: PathBuf = if under_language {
        path
    } else {
        Path::new(FUNCTIONS_DIR).join(language).join(path)
    };
    output.set_extension(SQL_EXTENSION);
    output
}
