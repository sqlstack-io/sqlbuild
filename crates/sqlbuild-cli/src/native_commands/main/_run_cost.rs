//! Persisted reports run natively; warehouse refresh belongs to the adapter edge.

use std::fs;
use std::path::Path;

use sqlbuild_core::cli_text::main::format_coded_error::format_coded_error;
use sqlbuild_core::cli_text::models::CliStyle;
use sqlbuild_core::cost_command::constants::{
    HISTORY_SELECTOR, LATEST_SELECTOR, REFRESHABLE_STATUSES,
};
use sqlbuild_core::cost_command::errors::CostCommandError;
use sqlbuild_core::cost_command::main::render_cost_report::render_cost_report;
use sqlbuild_core::cost_command::main::select_cost_run::select_cost_run;
use sqlbuild_core::cost_command::models::CostReport;
use sqlbuild_core::cost_report::models::CostRun;

use crate::cost_store::main::list_cost_runs::list_cost_runs;
use crate::native_commands::constants::RUNNING_BUILD_STATUS;
use crate::native_commands::errors::PythonNeeded;
use crate::native_commands::main::_cost_report_options::cost_report_options;
use crate::native_commands::models::{CommandOutput, CostOptions};

fn needs_refresh(runs: &[CostRun], selector: &str) -> Result<bool, CostCommandError> {
    if selector == HISTORY_SELECTOR {
        return Ok(false);
    }
    let selected = if selector == LATEST_SELECTOR {
        runs.iter()
            .find(|run| run.build_status != RUNNING_BUILD_STATUS)
    } else {
        let ids = runs
            .iter()
            .map(|run| run.run_id.clone())
            .collect::<Vec<_>>();
        runs.get(select_cost_run(&ids, selector)?)
    };
    Ok(selected.is_some_and(|run| REFRESHABLE_STATUSES.contains(&run.cost.status.as_str())))
}

fn refused(error: CostCommandError, use_color: bool) -> CommandOutput {
    let coded = format_coded_error(
        (error.code, &error.message, None),
        true,
        CliStyle { use_color },
    );
    CommandOutput {
        stdout: String::new(),
        stderr: format!("{coded}\n\n"),
        exit_code: 1,
    }
}

/// Render natively, or select the Python adapter edge before any side effect.
pub(crate) fn run_cost(
    project_dir: &Path,
    options: &CostOptions,
    error_color: bool,
) -> Result<CommandOutput, PythonNeeded> {
    let runs = list_cost_runs(project_dir);
    let request = match cost_report_options(options) {
        Ok(request) => request,
        Err(error) => return Ok(refused(error, error_color)),
    };
    match needs_refresh(&runs, &options.selector) {
        Ok(true) => return Err(PythonNeeded::Command),
        Ok(false) => {}
        Err(error) => return Ok(refused(error, error_color)),
    }
    let CostReport { json, text } = match render_cost_report(runs, &request) {
        Ok(report) => report,
        Err(error) => return Ok(refused(error, error_color)),
    };
    if let Some(path) = &options.json_path {
        let written = path
            .parent()
            .map_or(Ok(()), fs::create_dir_all)
            .and_then(|()| fs::write(path, format!("{json}\n")));
        if let Err(error) = written {
            return Ok(CommandOutput {
                stdout: String::new(),
                stderr: format!("sqb cost: cannot write {}: {error}\n", path.display()),
                exit_code: 1,
            });
        }
    }
    Ok(CommandOutput {
        stdout: format!("{}\n", if options.json { json } else { text }),
        ..CommandOutput::default()
    })
}
