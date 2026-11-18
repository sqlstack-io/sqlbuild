//! The jobs and groups a target declares, with each job's settings and retry.

use std::collections::BTreeMap;

use sqlbuild_discovery::orchestration_files::models::OrchestrationInventory;

use crate::job_settings::models::{DeclaredRun, JobRetry};

/// Each job's settings and retry, and each group's jobs, declared for `target`.
#[must_use]
pub fn declared_runs(
    inventory: &OrchestrationInventory,
    target: Option<&str>,
) -> BTreeMap<String, DeclaredRun> {
    let declared_for = |targets: &[String]| {
        targets.is_empty() || target.is_some_and(|target| targets.iter().any(|name| name == target))
    };
    let mut declared: BTreeMap<String, DeclaredRun> = BTreeMap::new();
    for (_, job) in &inventory.jobs {
        if declared_for(&job.run.targets) {
            declared.insert(
                job.run.name.value.clone(),
                DeclaredRun::Job {
                    settings: job.settings.clone(),
                    retry: job.run.retry.map(|retry| JobRetry {
                        attempts: retry.attempts,
                        delay_seconds: retry.delay_seconds,
                    }),
                },
            );
        }
    }
    for (_, group) in &inventory.groups {
        if declared_for(&group.run.targets) {
            let members: Vec<String> = group
                .jobs
                .iter()
                .filter(|job| declared.contains_key(&job.value))
                .map(|job| job.value.clone())
                .collect();
            declared.insert(group.run.name.value.clone(), DeclaredRun::Group { members });
        }
    }
    declared
}
