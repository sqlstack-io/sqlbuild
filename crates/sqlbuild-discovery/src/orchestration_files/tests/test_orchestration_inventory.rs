use std::path::Path;

use crate::orchestration_files::main::orchestration_inventory::orchestration_inventory;
use crate::orchestration_files::models::{OrchestrationFailure, OrchestrationInventory};
use crate::orchestration_files::tests::test_types::InventoryErrorCase;
use crate::tree::models::ProjectTree;

const EXPORT_JOB: &str = "name = \"orders_export_job\"\ndescription = \"d\"\nselect = \"orders\"\nschedule = { calendar = \"trading_days_calendar\", timezone = \"UTC\" }\n";
const NIGHTLY_GROUP: &str =
    "name = \"orders_nightly_job_group\"\ndescription = \"d\"\njobs = [\"orders_export_job\"]\n";
const CALENDAR: &str = "from sqlbuild.orchestration import calendar\n\n@calendar(name=\"trading_days_calendar\", description=\"d\", horizon=\"7d\")\ndef trading_days(window):\n    return []\n";
const SENSOR: &str = "from sqlbuild.orchestration import run_event, sensor\n\nstale = sensor(name=\"stock_stale_sensor\", description=\"d\", when=run_event(job=\"orders_export_job\", kinds=[]), run=\"orders_nightly_job_group\")\n";

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let directory = tempfile::tempdir().expect("temporary project");
    for (path, text) in files {
        let path = directory.path().join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folders");
        std::fs::write(path, text).expect("file");
    }
    directory
}

fn inventory(root: &Path) -> Result<OrchestrationInventory, OrchestrationFailure> {
    orchestration_inventory(&ProjectTree::new(root), (3, 12))
}

fn message(files: &[(&str, &str)]) -> (String, Option<String>) {
    let directory = project(files);
    match inventory(directory.path()).expect_err("invalid project") {
        OrchestrationFailure::Declaration(failure) => (failure.message, failure.help),
        other => panic!("unexpected failure {other:?}"),
    }
}

fn valid() -> Vec<(&'static str, &'static str)> {
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

#[test]
fn given_valid_layout_when_inventorying_then_reads_each_kind_and_names_them() {
    let directory = project(&valid());
    let found = inventory(directory.path()).expect("valid project");
    let names: Vec<(String, &str, String)> = found
        .names()
        .into_iter()
        .map(|fact| (fact.name, fact.kind, fact.path))
        .collect();
    assert_eq!(
        names,
        [
            (
                "orders_export_job".into(),
                "job",
                "orchestration/jobs/orders/export.toml".into()
            ),
            (
                "orders_nightly_job_group".into(),
                "job group",
                "orchestration/job_groups/orders/nightly.toml".into()
            ),
            (
                "trading_days_calendar".into(),
                "calendar",
                "orchestration/calendars/trading_days.py".into()
            ),
            (
                "stock_stale_sensor".into(),
                "sensor",
                "orchestration/sensors/inventory/stock.py".into()
            ),
        ]
    );
    assert_eq!(
        found.modules,
        [
            "orchestration/calendars/_holidays.py",
            "orchestration/calendars/trading_days.py",
            "orchestration/sensors/inventory/stock.py",
        ]
    );
}

#[test]
fn given_no_orchestration_folder_when_inventorying_then_is_empty() {
    let directory = project(&[("models/orders.sql", "select 1")]);
    assert!(
        inventory(directory.path())
            .expect("no declarations")
            .is_empty()
    );
}

#[test]
fn given_layout_and_reference_errors_when_inventorying_then_names_file_line_and_fix() {
    let mut cases: Vec<InventoryErrorCase> = Vec::new();
    let with = |path: &'static str, text: &'static str| {
        let mut files = valid();
        files.retain(|(existing, _)| *existing != path);
        files.push((path, text));
        files
    };
    cases.push(InventoryErrorCase {
        description: "loose file",
        files: with("orchestration/jobs.py", "x = 1\n"),
        expected_message: "orchestration/jobs.py:1:1: files cannot sit directly in orchestration/, except renames.toml",
        expected_help: Some("move it into one of orchestration/jobs/, orchestration/job_groups/, orchestration/calendars/, orchestration/sensors/"),
    });
    cases.push(InventoryErrorCase {
        description: "unknown folder",
        files: with("orchestration/schedules/nightly.toml", EXPORT_JOB),
        expected_message: "orchestration/schedules:1:1: unknown orchestration folder 'schedules'",
        expected_help: Some("use one of: jobs, job_groups, calendars, sensors"),
    });
    cases.push(InventoryErrorCase {
        description: "python job",
        files: with("orchestration/jobs/orders/legacy.py", "x = 1\n"),
        expected_message: "orchestration/jobs/orders/legacy.py:1:1: orchestration/jobs/ holds only .toml files",
        expected_help: Some("jobs and job groups are TOML; calendars and sensors are Python in orchestration/calendars/ and orchestration/sensors/"),
    });
    cases.push(InventoryErrorCase {
        description: "missing suffix",
        files: with(
            "orchestration/jobs/orders/export.toml",
            "name = \"orders_export\"\ndescription = \"d\"\nselect = \"orders\"\n",
        ),
        expected_message: "orchestration/jobs/orders/export.toml:1:8: job 'orders_export' must be named with the suffix '_job'",
        expected_help: Some("rename it to 'orders_export_job'"),
    });
    cases.push(InventoryErrorCase {
        description: "unknown group member",
        files: with(
            "orchestration/job_groups/orders/nightly.toml",
            "name = \"orders_nightly_job_group\"\ndescription = \"d\"\njobs = [\"orders_export_job\", \"carrier_sync_job\"]\n",
        ),
        expected_message: "orchestration/job_groups/orders/nightly.toml:3:30: job group 'orders_nightly_job_group' names 'carrier_sync_job', which is not a job in orchestration/jobs/",
        expected_help: None,
    });
    cases.push(InventoryErrorCase {
        description: "unknown calendar",
        files: with(
            "orchestration/calendars/trading_days.py",
            "from sqlbuild.orchestration import calendar\n\n@calendar(name=\"working_days_calendar\", description=\"d\", horizon=\"7d\")\ndef f(window):\n    return []\n",
        ),
        expected_message: "orchestration/jobs/orders/export.toml:4:25: schedule names 'trading_days_calendar', which is not a calendar in orchestration/calendars/",
        expected_help: None,
    });
    cases.push(InventoryErrorCase {
        description: "non-literal sensor job",
        files: with(
            "orchestration/sensors/inventory/stock.py",
            "from sqlbuild.orchestration import sensor\nJOB = \"orders_export_job\"\nstale = sensor(name=\"stock_stale_sensor\", description=\"d\", run=JOB)\n",
        ),
        expected_message: "orchestration/sensors/inventory/stock.py:3:64: sensor run job must be a string literal",
        expected_help: Some("write the sensor run job as a quoted string, such as \"orders_job\", so the compiler can check it without running the module"),
    });
    cases.push(InventoryErrorCase {
        description: "sensor in calendars",
        files: with("orchestration/calendars/stock.py", SENSOR),
        expected_message: "orchestration/calendars/stock.py:3:9: declares a sensor, which belongs in orchestration/sensors/",
        expected_help: None,
    });
    cases.push(InventoryErrorCase {
        description: "public module with nothing",
        files: with("orchestration/sensors/helpers.py", "def helper():\n    return 1\n"),
        expected_message: "orchestration/sensors/helpers.py:1:1: declares no sensor",
        expected_help: Some("declare one sensor, or start the file name with `_` to make it a private support module"),
    });
    cases.push(InventoryErrorCase {
        description: "removed Python job API",
        files: with(
            "orchestration/sensors/inventory/stock.py",
            "from sqlbuild.orchestration import job, sensor\nstale = sensor(name=\"stock_stale_sensor\", description=\"d\", run=\"orders_export_job\")\n",
        ),
        expected_message: "orchestration/sensors/inventory/stock.py:1:36: `job` was removed from sqlbuild.orchestration; jobs, job groups and schedules are declared in TOML",
        expected_help: Some("run `sqb migrate orchestration` to move Python jobs, job groups and schedules into TOML"),
    });
    let renamed = |text: &'static str| {
        let mut files = valid();
        files.push(("orchestration/renames.toml", text));
        files
    };
    cases.push(InventoryErrorCase {
        description: "rename to an undeclared job",
        files: renamed("[jobs]\norders_export = \"orders_exports_job\"\n"),
        expected_message: "orchestration/renames.toml:2:17: [jobs] renames 'orders_export' to 'orders_exports_job', which is not a declared job or job group",
        expected_help: None,
    });
    cases.push(InventoryErrorCase {
        description: "old name still declared",
        files: renamed("[jobs]\norders_export_job = \"orders_nightly_job_group\"\n"),
        expected_message: "orchestration/renames.toml:2:1: [jobs] renames 'orders_export_job', which is still declared",
        expected_help: Some("remove the pair, or finish the rename in the declaration"),
    });
    cases.push(InventoryErrorCase {
        description: "chained renames",
        files: renamed("[sensors]\nstock = \"stock_stale\"\nstock_stale = \"stock_stale_sensor\"\n"),
        expected_message: "orchestration/renames.toml:3:1: 'stock_stale' is both a new name and an old name; renames cannot chain",
        expected_help: Some("map each old name straight to its final name"),
    });
    cases.push(InventoryErrorCase {
        description: "two names onto one",
        files: renamed("[jobs]\norders = \"orders_export_job\"\nexport = \"orders_export_job\"\n"),
        expected_message: "orchestration/renames.toml:3:10: two names are renamed to 'orders_export_job'",
        expected_help: None,
    });
    cases.push(InventoryErrorCase {
        description: "trigger onto an unscheduled run",
        files: renamed("[triggers]\ntrading_days = \"orders_nightly_job_group\"\n"),
        expected_message: "orchestration/renames.toml:2:16: [triggers] renames 'trading_days' to 'orders_nightly_job_group', which is not a job or job group with a schedule",
        expected_help: None,
    });
    cases.push(InventoryErrorCase {
        description: "unknown renames section",
        files: renamed("[groups]\nnightly = \"orders_nightly_job_group\"\n"),
        expected_message: "orchestration/renames.toml:1:2: unknown renames file key 'groups'",
        expected_help: Some("supported keys: jobs, sensors, triggers"),
    });
    for case in cases {
        let (actual, actual_help) = message(&case.files);
        assert_eq!(actual, case.expected_message, "{}", case.description);
        assert_eq!(
            actual_help.as_deref(),
            case.expected_help,
            "{}",
            case.description
        );
    }
}

#[test]
fn given_renames_file_when_inventorying_then_reads_each_section_as_the_server_document() {
    let mut files = valid();
    files.push((
        "orchestration/renames.toml",
        "[jobs]\norders_export = \"orders_export_job\"\nnightly = \"orders_nightly_job_group\"\n\n[sensors]\nstock = \"stock_stale_sensor\"\n\n[triggers]\ntrading_days = \"orders_export_job\"\n",
    ));
    let directory = project(&files);
    let found = inventory(directory.path()).expect("valid renames");
    let (path, renames) = found.renames.expect("renames read");
    assert_eq!(path, "orchestration/renames.toml");
    assert_eq!(
        renames.document(),
        serde_json::json!({
            "jobs": {"orders_export": "orders_export_job", "nightly": "orders_nightly_job_group"},
            "sensors": {"stock": "stock_stale_sensor"},
            "triggers": {"trading_days": "orders_export_job"},
        })
    );
}
