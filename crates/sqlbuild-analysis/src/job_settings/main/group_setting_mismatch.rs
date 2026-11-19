//! A group's jobs must agree on `vars` and `changed` on each target that declares them.

use std::collections::{BTreeMap, BTreeSet};

use sqlbuild_discovery::orchestration_files::models::{GroupFile, OrchestrationInventory};

use crate::job_settings::constants::GROUP_AGREED_SETTINGS;
use crate::job_settings::main::build_job_settings::build_job_settings;
use crate::job_settings::main::declared_runs::declared_runs;
use crate::job_settings::models::DeclaredRun;

/// `run` keeping only the settings a group's jobs must agree on.
fn agreed_settings_only(run: DeclaredRun) -> DeclaredRun {
    match run {
        DeclaredRun::Job { settings, retry } => DeclaredRun::Job {
            settings: settings
                .into_iter()
                .filter(|(name, _)| GROUP_AGREED_SETTINGS.contains(&name.as_str()))
                .collect(),
            retry,
        },
        group => group,
    }
}

/// Why `group` cannot run as one build on some target, as building it would say; `None` if never.
#[must_use]
pub fn group_setting_mismatch(
    inventory: &OrchestrationInventory,
    group: &GroupFile,
) -> Option<String> {
    let mut targets: BTreeSet<Option<&str>> = BTreeSet::from([None]);
    targets.extend(group.run.targets.iter().map(|target| Some(target.as_str())));
    for (_, job) in &inventory.jobs {
        if group
            .jobs
            .iter()
            .any(|member| member.value == job.run.name.value)
        {
            targets.extend(job.run.targets.iter().map(|target| Some(target.as_str())));
        }
    }
    for target in targets {
        let declared: BTreeMap<String, DeclaredRun> = declared_runs(inventory, target)
            .into_iter()
            .map(|(name, run)| (name, agreed_settings_only(run)))
            .collect();
        let Some(DeclaredRun::Group { members }) = declared.get(&group.run.name.value) else {
            continue;
        };
        if let Err(conflict) = build_job_settings(members, &declared) {
            let on: String =
                target.map_or_else(String::new, |target| format!(" on target {target}"));
            return Some(format!(
                "{}{on}; group {} builds its jobs together",
                conflict.message, group.run.name.value
            ));
        }
    }
    None
}
