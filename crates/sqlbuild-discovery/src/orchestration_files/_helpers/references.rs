//! Name suffixes and the names groups, schedules and sensors refer to.

use std::collections::BTreeSet;

use crate::orchestration_files::constants::{
    CALENDAR_KIND, CALENDARS_DIRECTORY, GROUP_KIND, JOB_KIND, JOBS_DIRECTORY, SENSOR_KIND,
};
use crate::orchestration_files::models::{
    FileError, FilePosition, OrchestrationInventory, Positioned, RunFile, ScheduleSource,
};

/// The first problem with a name or reference, and the file it is in.
pub(crate) type ReferenceError = (String, FileError);

fn at(position: FilePosition, message: String, help: Option<String>) -> FileError {
    FileError {
        message,
        position,
        help,
    }
}

fn suffixed(
    path: &str,
    (noun, suffix): (&str, &str),
    name: &str,
    position: FilePosition,
) -> Result<(), ReferenceError> {
    if name.ends_with(suffix) && name.len() > suffix.len() {
        return Ok(());
    }
    Err((
        path.to_owned(),
        at(
            position,
            format!("{noun} '{name}' must be named with the suffix '{suffix}'"),
            Some(format!("rename it to '{name}{suffix}'")),
        ),
    ))
}

fn calendar_exists(
    path: &str,
    run: &RunFile,
    calendars: &BTreeSet<&str>,
) -> Result<(), ReferenceError> {
    let Some(ScheduleSource::Calendar(calendar)) = run.schedule.as_ref().map(|s| &s.source) else {
        return Ok(());
    };
    if calendars.contains(calendar.value.as_str()) {
        return Ok(());
    }
    Err((
        path.to_owned(),
        at(
            calendar.position,
            format!(
                "schedule names '{}', which is not a calendar in orchestration/{CALENDARS_DIRECTORY}/",
                calendar.value
            ),
            None,
        ),
    ))
}

fn member(
    path: &str,
    reference: &Positioned<String>,
    known: &BTreeSet<&str>,
    (owner, wanted): (&str, &str),
) -> Result<(), ReferenceError> {
    if known.contains(reference.value.as_str()) {
        return Ok(());
    }
    Err((
        path.to_owned(),
        at(
            reference.position,
            format!("{owner} names '{}', which is not {wanted}", reference.value),
            None,
        ),
    ))
}

/// Refuse a name without its kind's suffix, then a group member, calendar or sensor job that is not declared.
pub(crate) fn check_references(inventory: &OrchestrationInventory) -> Result<(), ReferenceError> {
    for (path, job) in &inventory.jobs {
        suffixed(path, JOB_KIND, &job.run.name.value, job.run.name.position)?;
    }
    for (path, group) in &inventory.groups {
        suffixed(
            path,
            GROUP_KIND,
            &group.run.name.value,
            group.run.name.position,
        )?;
    }
    for module in &inventory.calendars {
        suffixed(&module.path, CALENDAR_KIND, &module.name, module.position)?;
    }
    for module in &inventory.sensors {
        suffixed(&module.path, SENSOR_KIND, &module.name, module.position)?;
    }
    let jobs: BTreeSet<&str> = inventory
        .jobs
        .iter()
        .map(|(_, job)| job.run.name.value.as_str())
        .collect();
    let runnable: BTreeSet<&str> = jobs
        .iter()
        .copied()
        .chain(
            inventory
                .groups
                .iter()
                .map(|(_, group)| group.run.name.value.as_str()),
        )
        .collect();
    let calendars: BTreeSet<&str> = inventory
        .calendars
        .iter()
        .map(|module| module.name.as_str())
        .collect();
    for (path, job) in &inventory.jobs {
        calendar_exists(path, &job.run, &calendars)?;
    }
    for (path, group) in &inventory.groups {
        calendar_exists(path, &group.run, &calendars)?;
        let wanted = format!("a job in orchestration/{JOBS_DIRECTORY}/");
        for reference in &group.jobs {
            member(
                path,
                reference,
                &jobs,
                (&format!("job group '{}'", group.run.name.value), &wanted),
            )?;
        }
    }
    for module in &inventory.sensors {
        for (_, reference) in &module.job_references {
            member(
                &module.path,
                reference,
                &runnable,
                (
                    &format!("sensor '{}'", module.name),
                    "a declared job or job group",
                ),
            )?;
        }
    }
    Ok(())
}
