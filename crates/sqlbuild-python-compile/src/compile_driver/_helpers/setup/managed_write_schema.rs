//! Raise when neither the resource nor the project names a physical write schema.

use crate::compile_driver::_helpers::setup::targets::DEFAULTS_HELP_PURPOSE;
use crate::compile_driver::_helpers::setup::targets::DEFAULTS_SECTION;
use crate::compile_driver::_helpers::setup::targets::ignored_connection_note;
use crate::compile_driver::_helpers::setup::targets::planner_error;
use crate::compile_driver::_helpers::setup::targets::project_setting_help;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};
use sqlbuild_analysis::assembly::target_checks::constants::MANAGED_WRITE_SCHEMA_CODE;
use sqlbuild_analysis::assembly::target_checks::errors::TargetError;
use sqlbuild_analysis::assembly::target_checks::main::explicit_write_schema::explicit_write_schema;
use sqlbuild_analysis::assembly::target_checks::main::target_error_message::target_error_message;
use sqlbuild_python_base::boundary::main::optional_text::optional_text;

/// Raise when neither the resource nor the project names a physical write schema.
pub(crate) fn managed_write_schema(
    resource_label: &str,
    schema: &Bound<'_, PyAny>,
    (project_schema, ignored): (Option<&str>, Option<&str>),
) -> PyResult<()> {
    if explicit_write_schema(optional_text(schema)?.as_deref()).is_some()
        || project_schema.is_some()
    {
        return Ok(());
    }
    let py: Python<'_> = schema.py();
    let mut message: String =
        target_error_message(&TargetError::ManagedWriteSchema(resource_label.to_owned()));
    let help: Option<String> = match ignored {
        Some(value) => {
            message.push_str(&ignored_connection_note("schema", value));
            Some(project_setting_help(
                py,
                (DEFAULTS_HELP_PURPOSE, DEFAULTS_SECTION),
                ("schema", value),
            )?)
        }
        None => None,
    };
    Err(planner_error(py, message, MANAGED_WRITE_SCHEMA_CODE, help))
}
