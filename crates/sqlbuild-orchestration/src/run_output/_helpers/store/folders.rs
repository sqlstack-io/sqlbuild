//! Where run output goes when `[output] store` names nowhere.

use std::path::PathBuf;

use crate::run_output::constants::{
    DEFAULT_OUTPUT_FOLDER, HOME_STATE_FOLDER, NO_STATE_HOME, STATE_HOME_VARIABLE, STATE_SUBFOLDER,
};
use crate::store::errors::ControlError;
use crate::store::models::ControlStore;

/// `XDG_STATE_HOME`, then the user's home: `HOME` (`USERPROFILE` on Windows), else the account's own.
pub(crate) fn user_folders() -> (Option<PathBuf>, Option<PathBuf>) {
    (
        std::env::var_os(STATE_HOME_VARIABLE).map(PathBuf::from),
        std::env::home_dir(),
    )
}

/// `output` beside a SQLite control store; for PostgreSQL in the user's state folder, wherever the server started.
pub(crate) fn default_folder(
    control: &ControlStore,
    (state_home, home): (Option<PathBuf>, Option<PathBuf>),
) -> Result<PathBuf, ControlError> {
    match control {
        ControlStore::Sqlite(sqlite) => Ok(sqlite.path.parent().map_or_else(
            || PathBuf::from(DEFAULT_OUTPUT_FOLDER),
            |folder| folder.join(DEFAULT_OUTPUT_FOLDER),
        )),
        ControlStore::Postgres(_) => state_home
            .filter(|folder| folder.is_absolute())
            .or_else(|| {
                home.filter(|folder| folder.is_absolute())
                    .map(|folder| folder.join(HOME_STATE_FOLDER))
            })
            .map(|state| state.join(STATE_SUBFOLDER).join(DEFAULT_OUTPUT_FOLDER))
            .ok_or_else(|| ControlError::Invalid(NO_STATE_HOME.into())),
    }
}
