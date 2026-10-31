//! Validate frozen build settings against the same table as the command-line parser.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::argv::_helpers::job_settings::{setting_argument, validate_mutex};
use crate::argv::_helpers::normalization::cursor_error;
use crate::argv::_helpers::table::root_parser;
use crate::argv::constants::BUILD_COMMAND;

/// Produce argument tokens, never shell text; execution identity is owned by the run request.
pub fn job_settings_arguments(settings: &BTreeMap<String, Json>) -> Result<Vec<String>, String> {
    let parser = root_parser()
        .actions
        .iter()
        .flat_map(|action| &action.parsers)
        .find(|(name, _)| name == BUILD_COMMAND)
        .map(|(_, parser)| parser)
        .ok_or_else(|| "build command is unavailable".to_owned())?;
    let mut arguments: Vec<String> = Vec::new();
    for (name, value) in settings {
        if let Some(argument) = setting_argument(parser, name, value)? {
            arguments.push(argument);
        }
    }
    validate_mutex(parser, &arguments)?;
    if let Some(message) = cursor_error(|name| settings.contains_key(name)) {
        return Err(format!("job settings: {message}"));
    }
    Ok(arguments)
}
