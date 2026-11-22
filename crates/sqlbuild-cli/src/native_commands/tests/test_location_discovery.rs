use std::time::Duration;

use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_orchestration::discovered_sets::models::DiscoveredSetChecked;
use tokio::time::Instant;

use crate::native_commands::_helpers::location::discovery_state::LocationDiscovery;
use crate::native_commands::tests::helpers::{PLAN_FILE, discovered_load, failed_load, stamps};
use crate::native_commands::tests::test_types::{
    DiscoveryCheckTestCase, DiscoveryReportTestCase, FailedCompileDiscoveryTestCase,
    RediscoverTestCase,
};

const OTHER_FILE: &str = "python/assets/other.py";
const FIFTEEN_MINUTES: Duration = Duration::from_secs(900);

#[test]
fn given_a_published_location_when_it_publishes_again_then_only_edited_discover_modules_run_again()
{
    let test_cases = [
        RediscoverTestCase {
            description: "a SQL or unrelated edit reuses every set",
            changed: &[OTHER_FILE],
            everything: false,
            expected_rediscover: Some(&[]),
        },
        RediscoverTestCase {
            description: "an edit to the discover function's own module runs it again",
            changed: &[PLAN_FILE],
            everything: false,
            expected_rediscover: Some(&["orders_plan"]),
        },
        RediscoverTestCase {
            description: "r and the republish API discover everything",
            changed: &[],
            everything: true,
            expected_rediscover: None,
        },
    ];
    let before: Vec<PathStamp> = stamps(&[PLAN_FILE, OTHER_FILE], &[]);

    for test_case in test_cases {
        let mut discovery = LocationDiscovery::default();
        let first = discovery.publish_request(1, (&[], &before), 0);
        discovery.published(&discovered_load(&["a"]), Instant::now());
        let after: Vec<PathStamp> = stamps(&[PLAN_FILE, OTHER_FILE], test_case.changed);
        let next = discovery.publish_request(2, (&before, &after), u64::from(test_case.everything));
        let rediscover: Option<Vec<String>> =
            next.rediscover.map(|names| names.into_iter().collect());

        assert_eq!(
            (
                first.rediscover.is_none(),
                first.seed,
                rediscover,
                next.seed
            ),
            (
                true,
                true,
                test_case
                    .expected_rediscover
                    .map(|names| names.iter().map(|name| (*name).to_owned()).collect()),
                false
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_set_checked_every_15_minutes_when_its_check_finds_new_items_then_the_next_publication_reuses_them()
 {
    let test_cases = [
        DiscoveryCheckTestCase {
            description: "a changed result is kept for the next publication",
            checked_ids: &["a", "b"],
            changed: true,
            expected_early_check: false,
            expected_republish: true,
            expected_ids: &["a", "b"],
        },
        DiscoveryCheckTestCase {
            description: "an unchanged result keeps the published items",
            checked_ids: &["a"],
            changed: false,
            expected_early_check: false,
            expected_republish: false,
            expected_ids: &["a"],
        },
    ];
    let start = Instant::now();

    for test_case in test_cases {
        let mut discovery = LocationDiscovery::default();
        discovery.published(&discovered_load(&["a"]), start);
        let early: bool = discovery.due_check(start).is_some();
        let fresh = discovered_load(test_case.checked_ids).discovered["orders_plan"].clone();
        let republish = discovery.checked(
            "orders_plan",
            DiscoveredSetChecked {
                set: Some(fresh),
                changed: test_case.changed,
            },
            start + FIFTEEN_MINUTES,
        );
        let next = discovery.publish_request(2, (&[], &[]), 0);
        let ids: Vec<&str> = next.previous["orders_plan"]
            .result
            .iter()
            .flat_map(|result| &result.items)
            .map(|item| item.id.as_str())
            .collect();

        assert_eq!(
            (early, republish, ids),
            (
                test_case.expected_early_check,
                test_case.expected_republish,
                test_case.expected_ids.to_vec()
            ),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_failed_compile_when_the_location_retries_then_it_still_discovers_what_was_asked_for() {
    let test_cases = [
        FailedCompileDiscoveryTestCase {
            description: "an edit whose compile failed before discovery",
            edited_before_failure: &[PLAN_FILE],
            edited_after_failure: &[],
            everything: false,
            discovered_before_failing: false,
            expected_rediscover: Some(&["orders_plan"]),
        },
        FailedCompileDiscoveryTestCase {
            description: "an edit after a compile that failed following discovery",
            edited_before_failure: &[],
            edited_after_failure: &[PLAN_FILE],
            everything: false,
            discovered_before_failing: true,
            expected_rediscover: Some(&["orders_plan"]),
        },
        FailedCompileDiscoveryTestCase {
            description: "r whose compile failed",
            edited_before_failure: &[],
            edited_after_failure: &[],
            everything: true,
            discovered_before_failing: false,
            expected_rediscover: None,
        },
    ];
    let before: Vec<PathStamp> = stamps(&[PLAN_FILE], &[]);

    for test_case in test_cases {
        let asked: u64 = u64::from(test_case.everything);
        let failing: Vec<PathStamp> = stamps(&[PLAN_FILE], test_case.edited_before_failure);
        let edited: [&[&str]; 2] = [
            test_case.edited_before_failure,
            test_case.edited_after_failure,
        ];
        let retried: Vec<PathStamp> = stamps(&[PLAN_FILE], &edited.concat());
        let mut discovery = LocationDiscovery::default();
        discovery.publish_request(1, (&[], &before), 0);
        discovery.published(&discovered_load(&["a"]), Instant::now());
        discovery.publish_request(2, (&before, &failing), asked);
        discovery.published(
            &failed_load(test_case.discovered_before_failing),
            Instant::now(),
        );
        let rediscover: Option<Vec<String>> = discovery
            .publish_request(3, (&failing, &retried), asked)
            .rediscover
            .map(|names| names.into_iter().collect());

        assert_eq!(
            rediscover,
            test_case
                .expected_rediscover
                .map(|names| names.iter().map(|name| (*name).to_owned()).collect()),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_a_second_publication_when_reporting_sets_then_removing_the_last_one_clears_its_health() {
    let test_cases = [
        DiscoveryReportTestCase {
            description: "sets are reported",
            had_sets: true,
            has_sets: true,
            expected_report: true,
        },
        DiscoveryReportTestCase {
            description: "removing the last set reports none",
            had_sets: true,
            has_sets: false,
            expected_report: true,
        },
        DiscoveryReportTestCase {
            description: "a project without sets stays quiet",
            had_sets: false,
            has_sets: false,
            expected_report: false,
        },
    ];

    for test_case in test_cases {
        let mut first = discovered_load(&["a"]);
        first.discovered.retain(|_, _| test_case.had_sets);
        let mut second = discovered_load(&["a"]);
        second.discovered.retain(|_, _| test_case.has_sets);
        let mut discovery = LocationDiscovery::default();
        discovery.published(&first, Instant::now());

        assert_eq!(
            discovery.reports(&second),
            test_case.expected_report,
            "{}",
            test_case.description
        );
    }
}
