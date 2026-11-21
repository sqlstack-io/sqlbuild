use std::time::Duration;

use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_orchestration::discovered_sets::models::DiscoveredSetChecked;
use tokio::time::Instant;

use crate::native_commands::_helpers::location::discovery_state::LocationDiscovery;
use crate::native_commands::tests::helpers::{PLAN_FILE, discovered_load, stamps};
use crate::native_commands::tests::test_types::{DiscoveryCheckTestCase, RediscoverTestCase};

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
