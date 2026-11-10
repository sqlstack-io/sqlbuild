//! Turn frozen settings into one command's flags, leaving out settings that command does not define.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value as Json;

use crate::argv::_helpers::table::command_parser;
use crate::argv::main::command_settings_arguments::command_settings_arguments;

/// The flags `command` takes from `settings`; a setting it lacks is dropped rather than refused.
pub fn accepted_settings_arguments(
    command: &str,
    settings: &BTreeMap<String, Json>,
) -> Result<Vec<String>, String> {
    let defined: BTreeSet<&str> = command_parser(command)?
        .actions
        .iter()
        .filter_map(|action| action.dest.as_deref())
        .collect();
    let accepted: BTreeMap<String, Json> = settings
        .iter()
        .filter(|(name, _)| defined.contains(name.as_str()))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    command_settings_arguments(command, &accepted)
}
