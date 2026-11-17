//! Write the differences beside the compiler cache for inspection.

use crate::compile_delta::_helpers::verify::MISMATCH_FILE;
use std::path::Path;

/// Write the differences beside the compiler cache for inspection.
pub fn write_mismatch(cache_dir: &Path, differences: &[String]) {
    let _ = std::fs::create_dir_all(cache_dir);
    let text: String = serde_json::to_string_pretty(differences).unwrap_or_default();
    let _ = std::fs::write(cache_dir.join(MISMATCH_FILE), text);
}
