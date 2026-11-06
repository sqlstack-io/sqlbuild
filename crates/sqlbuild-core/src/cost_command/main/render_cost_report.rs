//! Build cost history or a selected run report directly from persisted native records.

use crate::cost_command::_helpers::sorting;
use crate::cost_command::constants::{HISTORY_SELECTOR, LATEST_SELECTOR};
use crate::cost_command::errors::CostCommandError;
use crate::cost_command::main::_cost_history_limit::cost_history_limit;
use crate::cost_command::main::select_cost_run::select_cost_run;
use crate::cost_command::main::validate_cost_request::validate_cost_request;
use crate::cost_command::models::{CostReport, CostReportOptions};
use crate::cost_report::main::cost_history_json::cost_history_json;
use crate::cost_report::main::cost_report_json::cost_report_json;
use crate::cost_report::main::format_cost_breakdown::format_cost_breakdown;
use crate::cost_report::main::format_cost_history::format_cost_history;
use crate::cost_report::main::select_cost_run::select_cost_run as latest_run;
use crate::cost_report::models::{BreakdownOptions, CostRun};

fn history(
    runs: Vec<CostRun>,
    options: &CostReportOptions,
) -> Result<CostReport, CostCommandError> {
    if options
        .since
        .zip(options.until)
        .is_some_and(|(since, until)| since > until)
    {
        return Err(CostCommandError::new(
            "C711",
            "cost --since must not be after --until",
        ));
    }
    let mut matching = Vec::with_capacity(runs.len());
    for run in runs {
        let completed = sorting::timestamp(&run.completed_at)?;
        if options.since.is_none_or(|since| completed >= since)
            && options.until.is_none_or(|until| {
                if options.until_exclusive {
                    completed < until
                } else {
                    completed <= until
                }
            })
        {
            matching.push(run);
        }
    }
    let matching_count = matching.len();
    let mut runs = sorting::history(matching, &options.sort, options.descending)?;
    if let Some(limit) = cost_history_limit(options.limit, options.no_limit) {
        runs.truncate(usize::try_from(limit).unwrap_or(0));
    }
    Ok(CostReport {
        json: cost_history_json(&runs, Some(matching_count)),
        text: format_cost_history(&runs, matching_count, options.use_color),
    })
}

fn detail(
    mut runs: Vec<CostRun>,
    options: &CostReportOptions,
) -> Result<CostReport, CostCommandError> {
    let mut run = if options.selector == LATEST_SELECTOR {
        latest_run(runs, LATEST_SELECTOR).ok_or_else(|| {
            CostCommandError::new("C704", "No persisted SQLBuild cost runs were found")
        })?
    } else {
        let ids = runs
            .iter()
            .map(|run| run.run_id.clone())
            .collect::<Vec<_>>();
        let index = select_cost_run(&ids, &options.selector)?;
        runs.remove(index)
    };
    run.cost.resources = sorting::resources(run.cost.resources, &options.sort, options.descending)?;
    let limit = if options.no_limit {
        None
    } else {
        options
            .limit
            .map(|limit| usize::try_from(limit).unwrap_or(usize::MAX))
    };
    let text = format_cost_breakdown(
        &run,
        BreakdownOptions {
            use_color: options.use_color,
            limit,
            terminal_width: options.terminal_width,
            show_run_id: true,
        },
    );
    if let Some(limit) = limit {
        run.cost.resources.truncate(limit);
    }
    Ok(CostReport {
        json: cost_report_json(&run),
        text,
    })
}

/// The selected report's JSON and text, with history filtering and detail-only row limits.
pub fn render_cost_report(
    runs: Vec<CostRun>,
    options: &CostReportOptions,
) -> Result<CostReport, CostCommandError> {
    validate_cost_request(
        &options.selector,
        (options.limit, options.no_limit),
        &options.sort,
        options.since.is_some() || options.until.is_some(),
    )?;
    if options.selector == HISTORY_SELECTOR {
        history(runs, options)
    } else {
        detail(runs, options)
    }
}
