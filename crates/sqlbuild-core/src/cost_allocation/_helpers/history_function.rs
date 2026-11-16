//! The Snowflake `QUERY_HISTORY` table function, qualified by database when there is one.

use crate::cost_allocation::constants::QUERY_HISTORY_FUNCTION;

/// `INFORMATION_SCHEMA.QUERY_HISTORY`, under `database` when it is given.
pub(crate) fn query_history_function(database: Option<&str>) -> String {
    match database {
        None => QUERY_HISTORY_FUNCTION.to_owned(),
        Some(database) => format!(
            "\"{}\".{QUERY_HISTORY_FUNCTION}",
            database.to_uppercase().replace('"', "\"\"")
        ),
    }
}
