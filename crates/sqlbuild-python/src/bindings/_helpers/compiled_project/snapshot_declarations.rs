//! Compile authoring records against retained graph data before publication.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::PyResult;
use serde::Deserialize;
use serde_json::Value;
use sqlbuild_analysis::graph::main::resolve_selectors::resolve_selectors;
use sqlbuild_analysis::graph::models::ProjectGraph;
use sqlbuild_orchestration::declarations::main::snapshot_definitions::snapshot_definitions;
use sqlbuild_orchestration::declarations::models::{
    JobDefinition, OnFailure, Schedule, TriggerDefinition, TriggerStatus,
};
use sqlbuild_orchestration::scheduler::models::{CatchUp, Overlap};
use sqlbuild_orchestration::snapshots::models::SnapshotInput;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredSchedule {
    cron: String,
    timezone: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthoredJob {
    name: String,
    description: String,
    select: String,
    exclude: String,
    overlap: Overlap,
    on_failure: OnFailure,
    concurrency_groups: Vec<String>,
    schedule: Option<AuthoredSchedule>,
    targets: Vec<String>,
}

pub(crate) fn compile_declarations(
    mut input: SnapshotInput,
    graph: &ProjectGraph,
    declarations: Vec<Value>,
) -> PyResult<SnapshotInput> {
    if declarations.is_empty() {
        return Ok(input);
    }
    if input.documents.contains_key("jobs") || input.documents.contains_key("triggers") {
        return Err(PyValueError::new_err(
            "authored declarations cannot overwrite snapshot job or trigger documents",
        ));
    }
    let mut jobs: BTreeMap<String, JobDefinition> = BTreeMap::new();
    let mut triggers: BTreeMap<String, TriggerDefinition> = BTreeMap::new();
    let mut names: BTreeSet<String> = BTreeSet::new();
    for declaration in declarations {
        let job: AuthoredJob = serde_json::from_value(declaration)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        if !names.insert(job.name.clone()) {
            return Err(PyValueError::new_err(format!("duplicate job {}", job.name)));
        }
        if !job.targets.is_empty() && !job.targets.contains(&input.target) {
            continue;
        }
        let tokens = |value: &str| {
            value
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        };
        if job.select.trim().is_empty() {
            return Err(PyValueError::new_err(format!(
                "job {} needs a selection",
                job.name
            )));
        }
        let selection: BTreeSet<String> =
            resolve_selectors(graph, &tokens(&job.select), &tokens(&job.exclude))
                .map_err(|error| PyValueError::new_err(error.message))?
                .into_iter()
                .map(|(kind, name)| format!("{kind}:{name}"))
                .collect();
        if let Some(schedule) = job.schedule {
            triggers.insert(
                job.name.clone(),
                TriggerDefinition {
                    description: job.description.clone(),
                    job_key: job.name.clone(),
                    schedule: Schedule::Cron {
                        expression: schedule.cron,
                        timezone: schedule.timezone,
                    },
                    starts_at: None,
                    default_status: TriggerStatus::Running,
                    catch_up: CatchUp::Latest,
                },
            );
        }
        jobs.insert(
            job.name,
            JobDefinition {
                description: job.description,
                selection,
                settings: BTreeMap::new(),
                overlap: job.overlap,
                on_failure: job.on_failure,
                concurrency_groups: job.concurrency_groups.into_iter().collect(),
            },
        );
    }
    for (key, document) in [
        ("jobs", serde_json::to_value(jobs)),
        ("triggers", serde_json::to_value(triggers)),
    ] {
        input.documents.insert(
            key.into(),
            document.map_err(|error| PyValueError::new_err(error.to_string()))?,
        );
    }
    snapshot_definitions(&input).map_err(|error| PyValueError::new_err(error.to_string()))?;
    Ok(input)
}
