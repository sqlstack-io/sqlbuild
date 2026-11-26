//! Plan, apply and load a SQLLoad project whose targets write through SQLBuild's sessions.

use std::collections::BTreeMap;

use sqlload::adapters::session::models::SessionConnection;
use sqlload::loader::main::apply_project::apply_project;
use sqlload::loader::main::connect_targets::connect_targets;
use sqlload::loader::main::load_tables::load_tables;
use sqlload::loader::main::plan_project::plan_project;
use sqlload::loader::models::{LoadRun, PlanOptions};
use sqlload::project::main::parse_python_project::parse_python_project;

use crate::runs::_helpers::stack::on_load_stack;
use crate::runs::_helpers::tables::{missing_tables, planned_tables};
use crate::runs::models::{LoadedTable, ProjectRun};

/// Each selected table's load of the project `text`; nothing loads when the plan lacks a `required` table.
pub fn run_project(
    text: &str,
    sessions: BTreeMap<String, SessionConnection>,
    (options, required): (PlanOptions, Vec<String>),
) -> Result<ProjectRun, String> {
    let project = parse_python_project(text, &|_| None, (&[], sessions))
        .map_err(|error| error.to_string())?;
    on_load_stack(|| async {
        let destinations = connect_targets(&project)
            .await
            .map_err(|error| error.to_string())?;
        let plan = plan_project(&project, &destinations, &options)
            .await
            .map_err(|error| error.to_string())?;
        if !plan.problems.is_empty() {
            return Err(plan.problems.join("\n"));
        }
        let planned: Vec<String> = planned_tables(&plan);
        let missing: Vec<String> = missing_tables(&required, &planned);
        if !missing.is_empty() {
            return Ok(ProjectRun {
                tables: Vec::new(),
                planned,
                missing,
            });
        }
        apply_project(&plan, &destinations)
            .await
            .map_err(|error| error.to_string())?;
        let run = LoadRun {
            project: &project,
            destinations: &destinations,
            host: None,
            options: &options,
        };
        let mut tables: Vec<LoadedTable> = Vec::new();
        for outcome in load_tables(&run, &plan, None).await {
            let (loaded, changes) = match outcome.loaded {
                Ok(loaded) => (Ok(loaded.rows), loaded.changes),
                Err(error) => (Err(error), Vec::new()),
            };
            tables.push(LoadedTable {
                table: outcome.table,
                loaded,
                changes,
                retryable: outcome.retryable,
            });
        }
        Ok(ProjectRun {
            tables,
            planned,
            missing,
        })
    })
}
