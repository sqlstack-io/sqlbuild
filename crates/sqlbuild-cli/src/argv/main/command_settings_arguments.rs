//! Turn frozen settings back into one command's flags, against the same table as the command-line parser.

use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::argv::_helpers::job_settings::{setting_argument, validate_mutex};
use crate::argv::_helpers::table::root_parser;

/// Produce argument tokens for `command`, never shell text; a setting the command lacks is refused.
pub fn command_settings_arguments(
    command: &str,
    settings: &BTreeMap<String, Json>,
) -> Result<Vec<String>, String> {
    let parser = root_parser()
        .actions
        .iter()
        .flat_map(|action| &action.parsers)
        .find(|(name, _)| name == command)
        .map(|(_, parser)| parser)
        .ok_or_else(|| format!("{command} command is unavailable"))?;
    let mut arguments: Vec<String> = Vec::new();
    for (name, value) in settings {
        if let Some(argument) = setting_argument(parser, name, value)? {
            arguments.push(argument);
        }
    }
    validate_mutex(parser, &arguments)?;
    Ok(arguments)
}
