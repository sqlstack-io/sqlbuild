//! The `--project-dir` and `--target` a command line names, read before the command runs.

use crate::argv::_helpers::job_settings::IntegerHost;
use crate::argv::constants::UNSHOWN_COLUMNS;
use crate::argv::main::namespace_defaults::namespace_defaults;
use crate::argv::main::parse_cli_arguments::parse_cli_arguments;
use crate::argv::models::{ArgvSettings, Value};

/// The named project directory and target as written, parsing numbers and defaults typed as the command's own parse does; `None` when the arguments don't parse, which the command reports.
pub(crate) fn command_project_and_target(
    arguments: &[String],
) -> Option<(Option<String>, Option<String>)> {
    let settings = ArgvSettings {
        columns: UNSHOWN_COLUMNS,
        use_color: false,
    };
    let outcome = parse_cli_arguments(arguments, namespace_defaults(), settings, &mut IntegerHost);
    let namespace = match outcome.parsed {
        Ok(namespace) => namespace,
        Err(_reported_by_the_command) => return None,
    };
    let text = |name: &str| match namespace.get(name) {
        Some(Value::Str(text) | Value::Path(text)) => Some(text.clone()),
        _ => None,
    };
    Some((text("project_dir"), text("target")))
}
