//! The files `sqb dev` writes inside a project, as the variables its location and workers read.

use std::path::{Path, PathBuf};

use sqlbuild_orchestration::store::errors::ControlError;
use sqlbuild_orchestration::store::models::ControlStore;

use crate::native_commands::constants::{DEVELOPMENT_CONTROL_FILE, DEVELOPMENT_OUTPUT_FOLDER};

/// The SQLite control store file and the run output folder `sqb dev` writes, as the variables naming them.
pub(crate) fn development_written(
    store: &ControlStore,
    output: Option<&Path>,
) -> Result<Vec<(&'static str, PathBuf)>, ControlError> {
    let mut written: Vec<(&'static str, PathBuf)> = Vec::new();
    if let ControlStore::Sqlite(path) = store {
        written.push((
            DEVELOPMENT_CONTROL_FILE,
            std::env::current_dir()?.join(path),
        ));
    }
    if let Some(folder) = output {
        written.push((DEVELOPMENT_OUTPUT_FOLDER, folder.to_path_buf()));
    }
    Ok(written)
}
