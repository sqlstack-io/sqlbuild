use std::path::Path;

use crate::models::{DiscoveryFailure, FailureKind};
use crate::orchestration_files::_helpers::read_group_file::read_group_file;
use crate::orchestration_files::main::orchestration_inventory::orchestration_inventory;
use crate::orchestration_files::models::{
    FilePosition, OrchestrationFailure, OrchestrationInventory, Positioned,
};
use crate::tree::models::ProjectTree;

pub(super) const EXPORT_JOB: &str = "name = \"orders_export_job\"\ndescription = \"d\"\nselect = \"orders\"\nschedule = { calendar = \"trading_days_calendar\", timezone = \"UTC\" }\n";
pub(super) const NIGHTLY_GROUP: &str =
    "name = \"orders_nightly_job_group\"\ndescription = \"d\"\njobs = [\"orders_export_job\"]\n";
pub(super) const CALENDAR: &str = "from sqlbuild.orchestration import calendar\n\n@calendar(name=\"trading_days_calendar\", description=\"d\", horizon=\"7d\")\ndef trading_days(window):\n    return []\n";
pub(super) const SENSOR: &str = "from sqlbuild.orchestration import run_event, sensor\n\nstale = sensor(name=\"stock_stale_sensor\", description=\"d\", when=run_event(job=\"orders_export_job\", kinds=[]), run=\"orders_nightly_job_group\")\n";

/// A 1-based line and column.
pub(super) fn at(line: usize, column: usize) -> FilePosition {
    FilePosition { line, column }
}

/// A string value written at `line` and `column`.
pub(super) fn written(value: &str, line: usize, column: usize) -> Positioned<String> {
    Positioned {
        value: value.to_owned(),
        position: at(line, column),
    }
}

/// A group's members with their positions, or its error's message and position.
pub(super) fn group_members(
    text: &str,
) -> Result<Vec<(String, FilePosition)>, (String, FilePosition)> {
    read_group_file(text)
        .map(|group| group.jobs.into_iter().map(member).collect())
        .map_err(|error| (error.message, error.position))
}

/// A group member's name and position.
fn member(job: Positioned<String>) -> (String, FilePosition) {
    (job.value, job.position)
}

/// A temporary project holding `files`.
pub(super) fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary project");
    for (path, text) in files {
        let path = directory.path().join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folders");
        std::fs::write(path, text).expect("file");
    }
    directory
}

/// The inventory of the project at `root`.
pub(super) fn inventory(root: &Path) -> Result<OrchestrationInventory, OrchestrationFailure> {
    orchestration_inventory(&ProjectTree::new(root), (3, 12))
}

/// Why the inventory of a project holding `files` fails.
pub(super) fn inventory_failure(files: &[(&str, &str)]) -> OrchestrationFailure {
    let directory = project(files);
    inventory(directory.path()).expect_err("invalid project")
}

/// A `D017` declaration failure with `message` and `help`.
pub(super) fn declaration_failure(message: &str, help: Option<&str>) -> OrchestrationFailure {
    OrchestrationFailure::Declaration(DiscoveryFailure {
        kind: FailureKind::Orchestration,
        message: message.to_owned(),
        help: help.map(str::to_owned),
    })
}

/// A declared name with its kind and file.
pub(super) fn named(name: &str, kind: &'static str, path: &str) -> (String, &'static str, String) {
    (name.to_owned(), kind, path.to_owned())
}

/// A project declaring one of each kind, plus files the inventory skips.
pub(super) fn valid() -> Vec<(&'static str, &'static str)> {
    vec![
        ("orchestration/jobs/orders/export.toml", EXPORT_JOB),
        (
            "orchestration/job_groups/orders/nightly.toml",
            NIGHTLY_GROUP,
        ),
        ("orchestration/calendars/trading_days.py", CALENDAR),
        ("orchestration/calendars/_holidays.py", "HOLIDAYS = []\n"),
        ("orchestration/sensors/inventory/stock.py", SENSOR),
        (
            "orchestration/sensors/__pycache__/stock.cpython-312.pyc",
            "x",
        ),
    ]
}

/// The valid project with the file at `path` replaced by, or added as, `text`.
pub(super) fn with_file(
    path: &'static str,
    text: &'static str,
) -> Vec<(&'static str, &'static str)> {
    let mut files: Vec<(&'static str, &'static str)> = valid();
    files.retain(|(existing, _)| *existing != path);
    files.push((path, text));
    files
}
