//! `written_entries`: the project's top-level entries that hold what `sqb dev` writes itself.

use std::path::{Path, PathBuf};

use sqlbuild_core::constants::{DEVELOPMENT_CONTROL_FILE_ENV, DEVELOPMENT_OUTPUT_FOLDER_ENV};

use crate::project_layout::_helpers::written::entries_holding;

/// The top-level entries of `root` holding the control store file and run output `sqb dev` names.
#[must_use]
pub fn written_entries(root: &Path) -> Vec<String> {
    let written: Vec<PathBuf> = [DEVELOPMENT_CONTROL_FILE_ENV, DEVELOPMENT_OUTPUT_FOLDER_ENV]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    entries_holding(root, &written)
}
