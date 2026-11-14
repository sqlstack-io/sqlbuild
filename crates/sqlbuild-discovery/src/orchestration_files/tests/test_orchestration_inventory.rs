use crate::orchestration_files::tests::helpers::{
    EXPORT_JOB, SENSOR, declaration_failure, inventory, inventory_failure, named, project, valid,
    with_file,
};
use crate::orchestration_files::tests::test_types::{InventoryErrorTestCase, InventoryTestCase};

#[test]
fn given_projects_when_inventorying_then_reads_each_kind_and_names_them() {
    let test_cases = [
        InventoryTestCase {
            description: "one declaration of each kind, skipping private modules and bytecode",
            files: valid(),
            expected_names: vec![
                named(
                    "orders_export_job",
                    "job",
                    "orchestration/jobs/orders/export.toml",
                ),
                named(
                    "orders_nightly_job_group",
                    "job group",
                    "orchestration/job_groups/orders/nightly.toml",
                ),
                named(
                    "trading_days_calendar",
                    "calendar",
                    "orchestration/calendars/trading_days.py",
                ),
                named(
                    "stock_stale_sensor",
                    "sensor",
                    "orchestration/sensors/inventory/stock.py",
                ),
            ],
            expected_modules: vec![
                "orchestration/calendars/_holidays.py",
                "orchestration/calendars/trading_days.py",
                "orchestration/sensors/inventory/stock.py",
            ],
            expected_empty: false,
        },
        InventoryTestCase {
            description: "no orchestration folder",
            files: vec![("models/orders.sql", "select 1")],
            expected_names: Vec::new(),
            expected_modules: Vec::new(),
            expected_empty: true,
        },
    ];
    for test_case in test_cases {
        let directory = project(&test_case.files);
        let found = inventory(directory.path()).expect(test_case.description);
        let names: Vec<(String, &str, String)> = found
            .names()
            .into_iter()
            .map(|fact| (fact.name, fact.kind, fact.path))
            .collect();
        assert_eq!(names, test_case.expected_names, "{}", test_case.description);
        assert_eq!(
            found.modules, test_case.expected_modules,
            "{}",
            test_case.description
        );
        assert_eq!(
            found.is_empty(),
            test_case.expected_empty,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_layout_and_reference_errors_when_inventorying_then_names_file_line_and_fix() {
    let test_cases = [
        InventoryErrorTestCase {
            description: "loose file",
            files: with_file("orchestration/jobs.py", "x = 1\n"),
            expected_message: "orchestration/jobs.py:1:1: files cannot sit directly in orchestration/",
            expected_help: Some(
                "jobs, job groups and schedules are TOML files in orchestration/jobs/ and orchestration/job_groups/; see https://sqlbuild.com/docs/concepts/orchestration/",
            ),
        },
        InventoryErrorTestCase {
            description: "loose non-Python file",
            files: with_file("orchestration/nightly.toml", EXPORT_JOB),
            expected_message: "orchestration/nightly.toml:1:1: files cannot sit directly in orchestration/",
            expected_help: Some(
                "move it into one of orchestration/jobs/, orchestration/job_groups/, orchestration/calendars/, orchestration/sensors/",
            ),
        },
        InventoryErrorTestCase {
            description: "unknown folder of Python modules",
            files: with_file("orchestration/pipelines/daily/orders.py", "x = 1\n"),
            expected_message: "orchestration/pipelines:1:1: unknown orchestration folder 'pipelines'",
            expected_help: Some(
                "jobs, job groups and schedules are TOML files in orchestration/jobs/ and orchestration/job_groups/; see https://sqlbuild.com/docs/concepts/orchestration/",
            ),
        },
        InventoryErrorTestCase {
            description: "unknown folder",
            files: with_file("orchestration/schedules/nightly.toml", EXPORT_JOB),
            expected_message: "orchestration/schedules:1:1: unknown orchestration folder 'schedules'",
            expected_help: Some("use one of: jobs, job_groups, calendars, sensors"),
        },
        InventoryErrorTestCase {
            description: "python job",
            files: with_file("orchestration/jobs/orders/legacy.py", "x = 1\n"),
            expected_message: "orchestration/jobs/orders/legacy.py:1:1: orchestration/jobs/ holds only .toml files",
            expected_help: Some(
                "jobs, job groups and schedules are TOML files in orchestration/jobs/ and orchestration/job_groups/; see https://sqlbuild.com/docs/concepts/orchestration/",
            ),
        },
        InventoryErrorTestCase {
            description: "non-TOML job file",
            files: with_file(
                "orchestration/jobs/orders/export.yaml",
                "name: orders_export_job\n",
            ),
            expected_message: "orchestration/jobs/orders/export.yaml:1:1: orchestration/jobs/ holds only .toml files",
            expected_help: Some(
                "jobs and job groups are TOML; calendars and sensors are Python in orchestration/calendars/ and orchestration/sensors/",
            ),
        },
        InventoryErrorTestCase {
            description: "missing suffix",
            files: with_file(
                "orchestration/jobs/orders/export.toml",
                "name = \"orders_export\"\ndescription = \"d\"\nselect = \"orders\"\n",
            ),
            expected_message: "orchestration/jobs/orders/export.toml:1:8: job 'orders_export' must be named with the suffix '_job'",
            expected_help: Some("rename it to 'orders_export_job'"),
        },
        InventoryErrorTestCase {
            description: "unknown group member",
            files: with_file(
                "orchestration/job_groups/orders/nightly.toml",
                "name = \"orders_nightly_job_group\"\ndescription = \"d\"\njobs = [\"orders_export_job\", \"carrier_sync_job\"]\n",
            ),
            expected_message: "orchestration/job_groups/orders/nightly.toml:3:30: job group 'orders_nightly_job_group' names 'carrier_sync_job', which is not a job in orchestration/jobs/",
            expected_help: None,
        },
        InventoryErrorTestCase {
            description: "unknown calendar",
            files: with_file(
                "orchestration/calendars/trading_days.py",
                "from sqlbuild.orchestration import calendar\n\n@calendar(name=\"working_days_calendar\", description=\"d\", horizon=\"7d\")\ndef f(window):\n    return []\n",
            ),
            expected_message: "orchestration/jobs/orders/export.toml:4:25: schedule names 'trading_days_calendar', which is not a calendar in orchestration/calendars/",
            expected_help: None,
        },
        InventoryErrorTestCase {
            description: "non-literal sensor job",
            files: with_file(
                "orchestration/sensors/inventory/stock.py",
                "from sqlbuild.orchestration import sensor\nJOB = \"orders_export_job\"\nstale = sensor(name=\"stock_stale_sensor\", description=\"d\", run=JOB)\n",
            ),
            expected_message: "orchestration/sensors/inventory/stock.py:3:64: sensor run job must be a string literal",
            expected_help: Some(
                "write the sensor run job as a quoted string, such as \"orders_job\", so the compiler can check it without running the module",
            ),
        },
        InventoryErrorTestCase {
            description: "sensor in calendars",
            files: with_file("orchestration/calendars/stock.py", SENSOR),
            expected_message: "orchestration/calendars/stock.py:3:9: declares a sensor, which belongs in orchestration/sensors/",
            expected_help: None,
        },
        InventoryErrorTestCase {
            description: "public module with nothing",
            files: with_file(
                "orchestration/sensors/helpers.py",
                "def helper():\n    return 1\n",
            ),
            expected_message: "orchestration/sensors/helpers.py:1:1: declares no sensor",
            expected_help: Some(
                "declare one sensor, or start the file name with `_` to make it a private support module",
            ),
        },
        InventoryErrorTestCase {
            description: "removed Python job API",
            files: with_file(
                "orchestration/sensors/inventory/stock.py",
                "from sqlbuild.orchestration import job, sensor\nstale = sensor(name=\"stock_stale_sensor\", description=\"d\", run=\"orders_export_job\")\n",
            ),
            expected_message: "orchestration/sensors/inventory/stock.py:1:36: `job` was removed from sqlbuild.orchestration; jobs, job groups and schedules are declared in TOML",
            expected_help: Some(
                "jobs, job groups and schedules are TOML files in orchestration/jobs/ and orchestration/job_groups/; see https://sqlbuild.com/docs/concepts/orchestration/",
            ),
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            inventory_failure(&test_case.files),
            declaration_failure(test_case.expected_message, test_case.expected_help),
            "{}",
            test_case.description
        );
    }
}
