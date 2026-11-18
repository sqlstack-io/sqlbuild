//! The settings a command freezes to, the same as `sqb server` freezes a run or plan request of it.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::adhoc::_helpers::planner::command_parts;

/// The validated settings `command` freezes to; selectors are not resolved.
pub(crate) fn command_settings(command: &[String]) -> Result<BTreeMap<String, Value>, String> {
    command_parts(command).map(|(_, _, settings)| settings)
}
