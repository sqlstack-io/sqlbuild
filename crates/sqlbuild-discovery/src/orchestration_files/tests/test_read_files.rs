use crate::orchestration_files::main::read_group_file::read_group_file;
use crate::orchestration_files::main::read_job_file::read_job_file;
use crate::orchestration_files::models::{
    FilePosition, InitialStatus, OnFailure, Overlap, ScheduleSource,
};

fn at(line: usize, column: usize) -> FilePosition {
    FilePosition { line, column }
}

const FULL_JOB: &str = r#"name = "orders_export_job"
description = "Write the daily partner file."
select = "python/orders/partner_export"
exclude = "tag:slow"
targets = ["production"]
owner = "Orders team"
overlap = "skip"
concurrency_groups = ["warehouse"]
schedule = { calendar = "trading_days_calendar", timezone = "Europe/London", default_status = "paused" }

[on_failure]
retries = 2

[resources]
memory = "2g"
cpus = 1.5

[settings]
vars = { batch = 23 }
concurrency = 4
"#;

#[test]
fn given_full_job_when_reading_then_types_every_field_and_keeps_positions() {
    let job = read_job_file(FULL_JOB).expect("valid job");
    assert_eq!(job.run.name.value, "orders_export_job");
    assert_eq!(job.run.name.position, at(1, 8));
    assert_eq!(job.select.position, at(3, 10));
    assert_eq!(
        job.exclude.map(|exclude| exclude.value).as_deref(),
        Some("tag:slow")
    );
    assert_eq!(job.run.overlap, Overlap::Skip);
    assert_eq!(
        job.run.on_failure,
        Some(OnFailure {
            retries: 2,
            retry_delay_seconds: 0
        })
    );
    let schedule = job.run.schedule.expect("schedule");
    assert_eq!(schedule.default_status, InitialStatus::Paused);
    let ScheduleSource::Calendar(calendar) = schedule.source else {
        panic!("calendar schedule expected");
    };
    assert_eq!(calendar.value, "trading_days_calendar");
    assert_eq!(calendar.position, at(9, 25));
    assert_eq!(
        job.resources
            .map(|resources| resources.value["cpus"].clone()),
        Some(serde_json::json!(1.5))
    );
    assert_eq!(job.settings["concurrency"], serde_json::json!(4));
    assert_eq!(job.settings["vars"], serde_json::json!({"batch": 23}));
}

#[test]
fn given_invalid_job_files_when_reading_then_reports_message_position_and_help() {
    let cases: [(&str, &str, &str, FilePosition, Option<&str>); 6] = [
        (
            "misspelt key",
            "name = \"a_job\"\ndescription = \"d\"\nselct = \"orders\"\n",
            "unknown job key 'selct'",
            at(3, 1),
            Some("did you mean 'select'?"),
        ),
        (
            "top-level concurrency",
            "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nconcurrency = 2\n",
            "unknown job key 'concurrency'",
            at(4, 1),
            Some("move it under [settings] as `concurrency = ...`"),
        ),
        (
            "blank select",
            "name = \"a_job\"\ndescription = \"d\"\nselect = \"  \"\n",
            "select must not be blank",
            at(3, 10),
            None,
        ),
        (
            "both cron and calendar",
            "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\nschedule = { cron = \"0 2 * * *\", calendar = \"c_calendar\", timezone = \"UTC\" }\n",
            "schedule needs exactly one of `cron` or `calendar`",
            at(4, 12),
            None,
        ),
        (
            "retries out of range",
            "name = \"a_job\"\ndescription = \"d\"\nselect = \"orders\"\non_failure = { retries = 0 }\n",
            "on_failure retries must be a whole number from 1 to 10",
            at(4, 14),
            None,
        ),
        (
            "missing description",
            "name = \"a_job\"\nselect = \"orders\"\n",
            "missing field `description`",
            at(1, 1),
            None,
        ),
    ];
    for (description, text, message, position, help) in cases {
        let error = read_job_file(text).expect_err(description);
        assert_eq!(error.message, message, "{description}");
        assert_eq!(error.position, position, "{description}");
        assert_eq!(error.help.as_deref(), help, "{description}");
    }
}

#[test]
fn given_group_when_reading_then_keeps_member_positions_and_refuses_repeats() {
    let group = read_group_file(
        "name = \"nightly_job_group\"\ndescription = \"d\"\njobs = [\"export_job\", \"carrier_job\"]\n",
    )
    .expect("valid group");
    let members: Vec<(&str, FilePosition)> = group
        .jobs
        .iter()
        .map(|job| (job.value.as_str(), job.position))
        .collect();
    assert_eq!(
        members,
        [("export_job", at(3, 9)), ("carrier_job", at(3, 23))]
    );

    let repeated = read_group_file(
        "name = \"nightly_job_group\"\ndescription = \"d\"\njobs = [\"export_job\", \"export_job\"]\n",
    )
    .expect_err("repeated member");
    assert_eq!(repeated.message, "job 'export_job' is listed twice");
    assert_eq!(repeated.position, at(3, 23));

    let selecting = read_group_file(
        "name = \"nightly_job_group\"\ndescription = \"d\"\njobs = [\"export_job\"]\nselect = \"orders\"\n",
    )
    .expect_err("groups do not select");
    assert_eq!(selecting.message, "unknown job group key 'select'");
}
