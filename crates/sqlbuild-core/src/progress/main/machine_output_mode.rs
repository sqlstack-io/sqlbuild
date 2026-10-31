//! Whether a command's stdout carries machine output, so human progress must use stderr.

use crate::progress::constants::{
    DBT_COMMAND, DBT_JSON_OPTION, JSON_FORMAT, LINEAGE_COMMAND, LOCATION_COMMAND, QUERY_COMMAND,
};
use crate::progress::models::OutputModeFacts;

/// `--json`, `sqb dbt ... --json`, `lineage --format json` or `query --format json`.
#[must_use]
pub fn machine_output_mode(facts: &OutputModeFacts) -> bool {
    let format = |value: &Option<String>| value.as_deref().unwrap_or_default().to_owned();
    match facts.command.as_deref() {
        _ if facts.json => true,
        Some(LOCATION_COMMAND) => true,
        Some(DBT_COMMAND) => facts.dbt_args.iter().any(|arg| arg == DBT_JSON_OPTION),
        Some(LINEAGE_COMMAND) => format(&facts.lineage_format) == JSON_FORMAT,
        Some(QUERY_COMMAND) => format(&facts.query_format) == JSON_FORMAT,
        _ => false,
    }
}
