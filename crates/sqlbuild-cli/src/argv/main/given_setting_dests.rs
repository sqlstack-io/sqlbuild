//! The settings a command line gives itself, which the settings of a job it names never override.

use std::collections::BTreeSet;

use crate::argv::_helpers::job_settings::IntegerHost;
use crate::argv::main::parse_cli_arguments::parse_cli_arguments;
use crate::argv::models::ArgvSettings;

/// The destination of every option `argv` writes, however abbreviated; `None` if it fails to parse.
#[must_use]
pub fn given_setting_dests(argv: &[String]) -> Option<BTreeSet<String>> {
    let mut host: IntegerHost = IntegerHost;
    let settings: ArgvSettings = ArgvSettings {
        columns: 80,
        use_color: false,
    };
    let outcome = parse_cli_arguments(argv, Vec::new(), settings, &mut host);
    match outcome.parsed {
        Ok(_) => Some(outcome.given),
        Err(_exit_status) => None,
    }
}
