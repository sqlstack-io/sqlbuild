use std::time::Duration;

use sqlbuild_cache::project_snapshot::models::PathStamp;
use sqlbuild_orchestration::discovered_sets::models::DiscoveredSetChecked;
use tokio::time::Instant;

#[cfg(unix)]
use crate::native_commands::_helpers::location::compiler_discovery::check_discovered;
use crate::native_commands::_helpers::location::discovery_state::LocationDiscovery;
#[cfg(unix)]
use crate::native_commands::models::{LocationCompileRequest, LocationRuntimeRequest};
use crate::native_commands::tests::helpers::{PLAN_FILE, discovered_load, failed_load, stamps};
#[cfg(unix)]
use crate::native_commands::tests::helpers::{scripted_compiler, silent_compiler};
#[cfg(unix)]
use crate::native_commands::tests::test_types::{CheckGraceTestCase, CheckShutdownTestCase};
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

#[cfg(unix)]
#[test]
fn given_a_check_that_never_returns_when_the_location_shuts_down_then_the_check_ends_promptly() {
    let test_cases = [
        CheckShutdownTestCase {
            description: "shutdown before the check starts",
            closes_after: Duration::ZERO,
            expected_within: Duration::from_secs(5),
            expected_compiler: false,
        },
        CheckShutdownTestCase {
            description: "shutdown during the check",
            closes_after: Duration::from_millis(300),
            expected_within: Duration::from_secs(5),
            expected_compiler: false,
        },
    ];

    for test_case in test_cases {
        let kept: bool = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                let mut load = discovered_load(&["a"]);
                for set in load.discovered.values_mut() {
                    set.check_every_seconds = Some(0);
                }
                let mut discovery = LocationDiscovery::default();
                discovery.published(&load, Instant::now());
                let (republish, _controls) = tokio::sync::watch::channel(Default::default());
                let (sender, requests) = tokio::sync::mpsc::channel(1);
                let check = check_discovered(
                    Some(silent_compiler()),
                    &mut discovery,
                    ((&republish, ("synthetic", uuid::Uuid::nil())), &requests),
                );
                let shutdown = async {
                    tokio::time::sleep(test_case.closes_after).await;
                    drop(sender);
                };
                tokio::time::timeout(test_case.expected_within, async {
                    tokio::join!(check, shutdown).0
                })
                .await
                .expect("the check ends promptly")
                .expect("the interpreter stops")
                .is_some()
            });

        assert_eq!(
            kept, test_case.expected_compiler,
            "{}",
            test_case.description
        );
    }
}

#[cfg(unix)]
#[test]
fn given_a_request_waiting_on_a_check_when_the_grace_passes_then_only_an_unfinished_check_is_abandoned()
 {
    let test_cases = [
        CheckGraceTestCase {
            description: "a check that never returns is abandoned after 10 seconds",
            script: "exec sleep 600",
            waiting: 1,
            expected_at_least: Duration::from_secs(10),
            expected_within: Duration::from_secs(15),
            expected_compiler: false,
        },
        CheckGraceTestCase {
            description: "a check that finishes inside the 10 seconds is kept",
            script: r#"read request; sleep 2; echo '{"set":null,"changed":false}'; exec sleep 600"#,
            waiting: 1,
            expected_at_least: Duration::from_secs(2),
            expected_within: Duration::from_secs(9),
            expected_compiler: true,
        },
        CheckGraceTestCase {
            description: "a slower check nothing waits for is kept",
            script: r#"read request; sleep 11; echo '{"set":null,"changed":false}'; exec sleep 600"#,
            waiting: 0,
            expected_at_least: Duration::from_secs(11),
            expected_within: Duration::from_secs(16),
            expected_compiler: true,
        },
    ];

    for test_case in test_cases {
        let (kept, elapsed): (bool, Duration) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(async {
                let mut load = discovered_load(&["a"]);
                for set in load.discovered.values_mut() {
                    set.check_every_seconds = Some(0);
                }
                let mut discovery = LocationDiscovery::default();
                discovery.published(&load, Instant::now());
                let (republish, _controls) = tokio::sync::watch::channel(Default::default());
                let (sender, requests) = tokio::sync::mpsc::channel(1);
                for generation in 0..test_case.waiting {
                    let (reply, _) = tokio::sync::oneshot::channel();
                    sender
                        .send(LocationRuntimeRequest::Publish(LocationCompileRequest {
                            generation: generation as i64,
                            python_sources: Vec::new(),
                            reply,
                        }))
                        .await
                        .expect("a request waits");
                }
                let started = Instant::now();
                let compiler = tokio::time::timeout(
                    test_case.expected_within,
                    check_discovered(
                        Some(scripted_compiler(test_case.script)),
                        &mut discovery,
                        ((&republish, ("synthetic", uuid::Uuid::nil())), &requests),
                    ),
                )
                .await
                .expect("the check ends in time")
                .expect("the interpreter is usable or stopped");
                (compiler.is_some(), started.elapsed())
            });

        assert_eq!(
            kept, test_case.expected_compiler,
            "{}",
            test_case.description
        );
        assert!(
            elapsed >= test_case.expected_at_least,
            "{}",
            test_case.description
        );
    }
}
