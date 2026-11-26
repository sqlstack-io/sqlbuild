//! The error when a scenario command needs SQL analysis that is turned off.

use crate::scenario_command::constants::SQL_VALIDATION_REQUIRED_CODE;
use crate::scenario_command::errors::ScenarioCommandError;
use crate::scenario_command::models::SqlAnalysisCommand;

/// Turned off by `--no-sql-analysis`, or by the config file `guidance` names as `(note, help)`.
#[must_use]
pub fn sql_analysis_required_error(
    command: SqlAnalysisCommand,
    disabled_by_flag: bool,
    guidance: Option<(String, String)>,
) -> Option<ScenarioCommandError> {
    let subject: &str = match command {
        SqlAnalysisCommand::LocalTest => "scenario test --run-on local",
        SqlAnalysisCommand::Capture => "scenario capture",
    };
    if disabled_by_flag {
        let help: &str = match command {
            SqlAnalysisCommand::LocalTest => "run the command without `--no-sql-analysis`",
            SqlAnalysisCommand::Capture => "run the capture without `--no-sql-analysis`",
        };
        return Some(ScenarioCommandError {
            code: SQL_VALIDATION_REQUIRED_CODE,
            message: format!(
                "{subject} requires SQL analysis, but `--no-sql-analysis` turns it off"
            ),
            help: Some(help.to_owned()),
        });
    }
    let (note, help) = guidance?;
    Some(ScenarioCommandError {
        code: SQL_VALIDATION_REQUIRED_CODE,
        message: format!("{subject} requires SQL analysis; {note}"),
        help: Some(help),
    })
}
