//! The note on a `delete_insert` model that matches rows by `unique_key`.

use crate::planner_strategy::constants::{DELETE_INSERT_STRATEGY, GROUP_KEY_NOTE_CODE};
use crate::planner_strategy::models::{MaterializationType, WarningRow, WarningSeverity};

/// The note naming `replace_key` for an incremental `delete_insert` model matched by `unique_key`.
pub fn group_key_note(
    model_name: &str,
    (materialization, strategy): (MaterializationType, Option<&str>),
    (has_unique_key, has_replace_key): (bool, bool),
) -> Option<WarningRow> {
    let matched_by_unique_key: bool = materialization == MaterializationType::Incremental
        && strategy == Some(DELETE_INSERT_STRATEGY)
        && has_unique_key
        && !has_replace_key;
    matched_by_unique_key.then(|| WarningRow {
        severity: WarningSeverity::Info,
        message: format!(
            "{model_name}: delete_insert matches rows by unique_key, which promises one row per \
             key; when a key value covers several rows, such as a version, set replace_key instead"
        ),
        code: Some(GROUP_KEY_NOTE_CODE),
    })
}
