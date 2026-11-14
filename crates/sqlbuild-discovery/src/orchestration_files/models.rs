//! Typed job and job group files.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::models::{DiscoveryFailure, ReadFailure, StageFailure};
use crate::project_validation::models::OrchestrationNameFact;

/// Whether a run that starts while one is active waits (`queue`) or is dropped (`skip`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Overlap {
    #[default]
    Queue,
    Skip,
}

/// Whether a schedule starts running or paused when first published.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InitialStatus {
    #[default]
    Running,
    Paused,
}

/// Reruns from failure, `retries` times, `retry_delay_seconds` apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnFailure {
    pub retries: u8,
    pub retry_delay_seconds: u32,
}

/// A 1-based line and column in a declaration file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct FilePosition {
    pub line: usize,
    pub column: usize,
}

/// A value and where it is written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Positioned<T> {
    pub value: T,
    pub position: FilePosition,
}

/// Why a declaration file cannot be read, at the position it points to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileError {
    pub message: String,
    pub position: FilePosition,
    pub help: Option<String>,
}

/// What starts a job or group on its own: a cron expression or a named calendar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleSource {
    Cron(String),
    Calendar(Positioned<String>),
}

/// A job's or group's `schedule` table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScheduleFile {
    pub source: ScheduleSource,
    pub timezone: String,
    /// RFC 3339 as written; publication parses it.
    pub starts_at: Option<String>,
    pub default_status: InitialStatus,
}

/// The fields jobs and groups share.
#[derive(Clone, Debug, PartialEq)]
pub struct RunFile {
    pub name: Positioned<String>,
    pub description: String,
    pub targets: Vec<String>,
    pub owner: Option<String>,
    pub overlap: Overlap,
    pub concurrency_groups: Vec<String>,
    pub schedule: Option<ScheduleFile>,
    pub on_failure: Option<OnFailure>,
}

/// One `orchestration/jobs/**.toml` file.
#[derive(Clone, Debug, PartialEq)]
pub struct JobFile {
    pub run: RunFile,
    pub select: Positioned<String>,
    pub exclude: Option<Positioned<String>>,
    /// Worker container limits as written; publication checks their values.
    pub resources: Option<Positioned<serde_json::Map<String, serde_json::Value>>>,
    /// `sqb build` flags by name, as JSON values.
    pub settings: BTreeMap<String, serde_json::Value>,
}

/// One `orchestration/job_groups/**.toml` file.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupFile {
    pub run: RunFile,
    pub jobs: Vec<Positioned<String>>,
}

/// A calendar or sensor module and the declaration it makes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedModule {
    pub name: String,
    pub path: String,
    pub position: FilePosition,
    /// The jobs and groups a sensor starts (`run`) or watches (`run_event`); empty for a calendar.
    pub job_references: Vec<(&'static str, Positioned<String>)>,
    /// A sensor's targets: empty for every target, `None` when they are not written as literals.
    pub targets: Option<Vec<String>>,
}

/// One section of the renames file: each old name and the name it became, where each is written.
pub type RenamePairs = Vec<(Positioned<String>, Positioned<String>)>;

/// The renames file: old names of jobs and groups, sensors, and schedule triggers, and their new names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RenameMap {
    pub jobs: RenamePairs,
    pub sensors: RenamePairs,
    pub triggers: RenamePairs,
}

impl RenameMap {
    /// The map as the snapshot document the server applies: `{section: {old: new}}`.
    #[must_use]
    pub fn document(&self) -> serde_json::Value {
        let section = |pairs: &RenamePairs| {
            pairs
                .iter()
                .map(|(old, new)| {
                    (
                        old.value.clone(),
                        serde_json::Value::String(new.value.clone()),
                    )
                })
                .collect::<serde_json::Map<_, _>>()
        };
        serde_json::json!({
            "jobs": section(&self.jobs),
            "sensors": section(&self.sensors),
            "triggers": section(&self.triggers),
        })
    }
}

/// Every orchestration declaration in a project, checked for layout, names and references.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OrchestrationInventory {
    pub jobs: Vec<(String, JobFile)>,
    pub groups: Vec<(String, GroupFile)>,
    pub calendars: Vec<NamedModule>,
    pub sensors: Vec<NamedModule>,
    /// Every Python module under `calendars/` and `sensors/`, which publication imports.
    pub modules: Vec<String>,
    /// The renames file and its path, when the project has one.
    pub renames: Option<(String, RenameMap)>,
}

impl OrchestrationInventory {
    /// Each declared name with its kind and file, for the project-wide name check.
    #[must_use]
    pub fn names(&self) -> Vec<OrchestrationNameFact> {
        let fact = |name: &str, kind: (&'static str, &str), path: &str| OrchestrationNameFact {
            name: name.to_owned(),
            kind: kind.0,
            path: path.to_owned(),
        };
        use crate::orchestration_files::constants::{
            CALENDAR_KIND, GROUP_KIND, JOB_KIND, SENSOR_KIND,
        };
        let mut names: Vec<OrchestrationNameFact> = Vec::new();
        names.extend(
            self.jobs
                .iter()
                .map(|(path, job)| fact(&job.run.name.value, JOB_KIND, path)),
        );
        names.extend(
            self.groups
                .iter()
                .map(|(path, group)| fact(&group.run.name.value, GROUP_KIND, path)),
        );
        names.extend(
            self.calendars
                .iter()
                .map(|module| fact(&module.name, CALENDAR_KIND, &module.path)),
        );
        names.extend(
            self.sensors
                .iter()
                .map(|module| fact(&module.name, SENSOR_KIND, &module.path)),
        );
        names
    }

    /// Whether the project declares anything to orchestrate.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty() && self.groups.is_empty() && self.sensors.is_empty()
    }
}

/// Why the orchestration declarations cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrchestrationFailure {
    /// A layout, file, name or reference error, as `OrchestrationDeclarationError`.
    Declaration(DiscoveryFailure),
    /// A file could not be read the way Python's `read_text` reads it.
    Unreadable {
        relative_path: String,
        failure: ReadFailure,
    },
    Stage(StageFailure),
}

impl From<StageFailure> for OrchestrationFailure {
    fn from(failure: StageFailure) -> Self {
        Self::Stage(failure)
    }
}
