use chrono::{DateTime, SecondsFormat};

use crate::scheduler::main::_next_cron_time::next_cron_time;
use crate::scheduler::main::_previous_cron_time::previous_cron_time;
use crate::scheduler::main::parse_cron::parse_cron;
use crate::scheduler::tests::test_types::{CronTestCase, PreviousCronTestCase};

#[test]
fn given_cron_and_iana_time_zone_when_crossing_dst_then_utc_ticks_follow_the_explicit_policy() {
    let test_cases = [
        CronTestCase {
            description: "spring gap ends at 03:00, not 03:30",
            expression: "30 2 * * *",
            timezone: "America/New_York",
            after: "2026-03-08T00:00:00Z",
            expected_next: "2026-03-08T07:00:00Z",
        },
        CronTestCase {
            description: "ambiguous daily time uses the later fold",
            expression: "30 1 * * *",
            timezone: "America/New_York",
            after: "2026-11-01T00:00:00Z",
            expected_next: "2026-11-01T06:30:00Z",
        },
        CronTestCase {
            description: "restarting in the first fold does not skip a smaller future wall time",
            expression: "30 1 * * *",
            timezone: "America/New_York",
            after: "2026-11-01T05:45:00Z",
            expected_next: "2026-11-01T06:30:00Z",
        },
        CronTestCase {
            description: "a persisted later-fold cursor does not repeat the tick",
            expression: "30 1 * * *",
            timezone: "America/New_York",
            after: "2026-11-01T06:30:00Z",
            expected_next: "2026-11-02T06:30:00Z",
        },
        CronTestCase {
            description: "wildcard schedules also select only the later fold",
            expression: "* * * * *",
            timezone: "America/New_York",
            after: "2026-11-01T05:45:00Z",
            expected_next: "2026-11-01T06:00:00Z",
        },
        CronTestCase {
            description: "half-hour spring gap moves to the actual gap end",
            expression: "15 2 * * *",
            timezone: "Australia/Lord_Howe",
            after: "2026-10-03T13:00:00Z",
            expected_next: "2026-10-03T15:30:00Z",
        },
        CronTestCase {
            description: "half-hour autumn fold uses the later 01:45",
            expression: "45 1 * * *",
            timezone: "Australia/Lord_Howe",
            after: "2026-04-04T13:00:00Z",
            expected_next: "2026-04-04T15:15:00Z",
        },
        CronTestCase {
            description: "an entire skipped date moves to the first instant of the next date",
            expression: "0 12 * * *",
            timezone: "Pacific/Apia",
            after: "2011-12-30T00:00:00Z",
            expected_next: "2011-12-30T10:00:00Z",
        },
        CronTestCase {
            description: "POSIX day-of-month and weekday are alternatives",
            expression: "0 9 13 * MON",
            timezone: "UTC",
            after: "2026-10-10T12:00:00Z",
            expected_next: "2026-10-12T09:00:00Z",
        },
        CronTestCase {
            description: "UTC schedules do not introduce local-zone shifts",
            expression: "17 6 * * *",
            timezone: "UTC",
            after: "2026-03-08T06:17:00Z",
            expected_next: "2026-03-09T06:17:00Z",
        },
    ];
    for test_case in test_cases {
        let schedule =
            parse_cron(test_case.expression, test_case.timezone).expect("valid synthetic cron");
        let after = DateTime::parse_from_rfc3339(test_case.after)
            .expect("valid start timestamp")
            .to_utc();
        let observed = next_cron_time(&schedule, after).expect("next cron time exists");
        assert_eq!(
            observed.to_rfc3339_opts(SecondsFormat::Secs, true),
            test_case.expected_next,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_latest_tick_search_when_crossing_folds_and_gaps_then_it_agrees_with_forward_tick_policy()
{
    let test_cases = [
        PreviousCronTestCase {
            description: "first fold excludes every ambiguous minute",
            expression: "* * * * *",
            timezone: "America/New_York",
            at: "2026-11-01T05:45:00Z",
            expected_previous: "2026-11-01T04:59:00Z",
        },
        PreviousCronTestCase {
            description: "second fold includes the later ambiguous instant",
            expression: "* * * * *",
            timezone: "America/New_York",
            at: "2026-11-01T06:15:00Z",
            expected_previous: "2026-11-01T06:15:00Z",
        },
        PreviousCronTestCase {
            description: "daily missing time fires exactly at the gap end",
            expression: "30 2 * * *",
            timezone: "America/New_York",
            at: "2026-03-08T07:00:00Z",
            expected_previous: "2026-03-08T07:00:00Z",
        },
        PreviousCronTestCase {
            description: "before gap end uses the preceding day",
            expression: "30 2 * * *",
            timezone: "America/New_York",
            at: "2026-03-08T06:59:59Z",
            expected_previous: "2026-03-07T07:30:00Z",
        },
        PreviousCronTestCase {
            description: "half-hour gap resolves to its actual end",
            expression: "15 2 * * *",
            timezone: "Australia/Lord_Howe",
            at: "2026-10-03T15:30:00Z",
            expected_previous: "2026-10-03T15:30:00Z",
        },
        PreviousCronTestCase {
            description: "skipped-date cron fires at the first representable instant",
            expression: "0 12 * * *",
            timezone: "Pacific/Apia",
            at: "2011-12-30T10:00:00Z",
            expected_previous: "2011-12-30T10:00:00Z",
        },
    ];
    for test_case in test_cases {
        let cron = parse_cron(test_case.expression, test_case.timezone).expect("valid cron");
        let at = DateTime::parse_from_rfc3339(test_case.at)
            .expect("search time")
            .to_utc();
        let previous = previous_cron_time(&cron, at).expect("previous scheduled instant");
        assert_eq!(
            previous.to_rfc3339_opts(SecondsFormat::Secs, true),
            test_case.expected_previous,
            "{}",
            test_case.description
        );
    }
}
