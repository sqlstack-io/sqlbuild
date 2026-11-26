//! SQLLoad's plan of a project whose targets write through SQLBuild's sessions, without applying it.

use std::collections::BTreeMap;

use sqlload::adapters::session::models::SessionConnection;
use sqlload::loader::main::connect_targets::connect_targets;
use sqlload::loader::main::plan_project::plan_project as plan_sqlload_project;
use sqlload::loader::main::render_plan::render_plan;
use sqlload::loader::models::PlanOptions;
use sqlload::project::main::parse_python_project::parse_python_project;

use crate::runs::_helpers::stack::on_load_stack;
use crate::runs::_helpers::tables::{missing_tables, planned_tables, shown_plan_lines};
use crate::runs::models::PlannedProject;

/// The plan of the project `text`: its lines, readiness, destination tables, and the `required` ones it lacks.
pub fn plan_project(
    text: &str,
    sessions: BTreeMap<String, SessionConnection>,
    (options, required): (PlanOptions, Vec<String>),
) -> Result<PlannedProject, String> {
    let project = parse_python_project(text, &|_| None, (&[], sessions))
        .map_err(|error| error.to_string())?;
    on_load_stack(|| async {
        let destinations = connect_targets(&project)
            .await
            .map_err(|error| error.to_string())?;
        let plan = plan_sqlload_project(&project, &destinations, &options)
            .await
            .map_err(|error| error.to_string())?;
        let (lines, ready) = render_plan(&plan, false);
        let tables: Vec<String> = planned_tables(&plan);
        Ok(PlannedProject {
            lines: shown_plan_lines(lines),
            ready,
            missing: missing_tables(&required, &tables),
            tables,
        })
    })
}
