//! Validate the `warehouse` a job's `[settings]` gives its nodes.

use sqlbuild_core::text::main::python_str_repr::python_str_repr;
use sqlbuild_core::text::main::python_strip::python_strip;

use crate::command_connection::_helpers::identifiers::is_warehouse_identifier;
use crate::command_connection::constants::WAREHOUSE_ERROR_CODE;
use crate::command_connection::errors::CommandConnectionError;

/// The warehouse name of `job`'s `warehouse` setting; `value` is `None` when the setting is not text.
pub fn check_job_warehouse(
    job: &str,
    value: Option<&str>,
) -> Result<String, CommandConnectionError> {
    let problem: String = match value {
        Some(name) if is_warehouse_identifier(name) => return Ok(name.to_owned()),
        Some(name) if python_strip(name).is_empty() => "an empty warehouse name".to_owned(),
        Some(name) => format!(
            "warehouse {}, which is not a valid Snowflake warehouse identifier",
            python_str_repr(name)
        ),
        None => "a warehouse that is not text".to_owned(),
    };
    Err(CommandConnectionError::User {
        code: Some(WAREHOUSE_ERROR_CODE),
        message: format!("job {job} sets {problem}"),
        help: Some(
            "set warehouse in the job's [settings] to an unquoted identifier such as \"ADHOC_WH\", or a double-quoted one such as '\"Adhoc WH\"'"
                .to_owned(),
        ),
    })
}
