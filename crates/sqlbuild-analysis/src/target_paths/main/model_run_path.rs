//! Where a model's executed SQL is written under `target/run`.

use crate::target_paths::constants::MODELS_DIR;

/// The model's relative path, under `sql/models/` unless it already starts there.
#[must_use]
pub fn model_run_path(relative_path: &str) -> String {
    if relative_path
        .strip_prefix(MODELS_DIR)
        .is_some_and(|rest| rest.starts_with('/'))
    {
        return relative_path.to_owned();
    }
    format!("{MODELS_DIR}/{relative_path}")
}
