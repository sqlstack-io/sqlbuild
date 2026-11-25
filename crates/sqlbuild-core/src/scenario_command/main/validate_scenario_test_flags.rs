//! `sqb scenario test` flag combinations rejected before any scenario runs.

use crate::scenario_command::constants::{
    LOCAL_RETAIN_UNSUPPORTED_CODE, LOCAL_SNAPSHOT_FLAG_REQUIRED_CODE,
};
use crate::scenario_command::errors::ScenarioCommandError;
use crate::scenario_command::models::ScenarioTestFlags;

/// `--run-on local` cannot retain, and snapshot sync flags need scenarios that can run locally.
pub fn validate_scenario_test_flags(flags: ScenarioTestFlags) -> Result<(), ScenarioCommandError> {
    if flags.local && flags.retain {
        return Err(ScenarioCommandError {
            code: LOCAL_RETAIN_UNSUPPORTED_CODE,
            message: "scenario test --run-on local does not support --retain".to_owned(),
            help: Some(
                "Local scenario DuckDB files are always kept under target/run/scenarios/."
                    .to_owned(),
            ),
        });
    }
    if flags.warehouse && (flags.sync_snapshots || flags.refresh) {
        return Err(ScenarioCommandError {
            code: LOCAL_SNAPSHOT_FLAG_REQUIRED_CODE,
            message: "scenario snapshot sync flags need scenarios that run locally".to_owned(),
            help: Some(
                "Use --run-on auto or --run-on local with --sync-snapshots or --refresh."
                    .to_owned(),
            ),
        });
    }
    Ok(())
}
