//! Validate frozen build settings against the same table as the command-line parser.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::argv::_helpers::normalization::cursor_error;
use crate::argv::constants::{BUILD_COMMAND, IGNORE_AUDIT_FAILURES_DEST, RUN_AUDITS_DEST};
use crate::argv::main::command_settings_arguments::command_settings_arguments;

/// Produce argument tokens, never shell text; execution identity is owned by the run request.
pub fn job_settings_arguments(settings: &BTreeMap<String, Json>) -> Result<Vec<String>, String> {
    let arguments = command_settings_arguments(BUILD_COMMAND, settings)?;
    if let Some(message) = cursor_error(|name| settings.contains_key(name)) {
        return Err(format!("job settings: {message}"));
    }
    if settings.get(IGNORE_AUDIT_FAILURES_DEST) == Some(&Json::Bool(true))
        && settings.get(RUN_AUDITS_DEST) == Some(&Json::Bool(false))
    {
        return Err(
            "job settings: ignore_audit_failures cannot be combined with run_audits = false"
                .to_owned(),
        );
    }
    Ok(arguments)
}
