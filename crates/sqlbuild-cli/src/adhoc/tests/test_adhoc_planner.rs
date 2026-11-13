//! The planner parses with the CLI's own table, refuses every flag outside the allowlist and resolves exact keys.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use sqlbuild_orchestration::runs::types::PublishedJobs;

use crate::adhoc::main::adhoc_planner::adhoc_planner;
use crate::adhoc::tests::helpers::{owned_plan, resource};
use crate::adhoc::tests::test_types::AdhocPlanTestCase;

#[test]
fn given_ad_hoc_commands_when_planned_then_flags_follow_the_allowlist_and_selectors_resolve_exactly()
 {
    let resources = [
        resource(
            "model:orders",
            &["source:shop.raw_orders", "seed:currencies"],
            &["daily"],
            Some("models/marts"),
        ),
        resource(
            "model:order_totals",
            &["model:orders"],
            &["daily"],
            Some("models/marts"),
        ),
        resource("model:customers", &[], &[], Some("models/staging")),
        resource("source:shop.raw_orders", &[], &[], None),
        resource("seed:currencies", &[], &[], None),
    ];
    let jobs: PublishedJobs = BTreeMap::from([
        (
            "nightly".to_owned(),
            BTreeSet::from(["model:orders".to_owned(), "model:order_totals".to_owned()]),
        ),
        (
            "daily_group".to_owned(),
            BTreeSet::from(["model:customers".to_owned(), "model:orders".to_owned()]),
        ),
    ]);
    let test_cases = [
        AdhocPlanTestCase {
            description: "a published job selects its published nodes",
            command: &[
                "build",
                "--select",
                "job:nightly",
                "--exclude",
                "order_totals",
            ],
            expected_plan: Ok((&["model:orders"], json!({}))),
        },
        AdhocPlanTestCase {
            description: "a bare published group name needs no prefix",
            command: &["build", "--select", "daily_group"],
            expected_plan: Ok((&["model:customers", "model:orders"], json!({}))),
        },
        AdhocPlanTestCase {
            description: "an unpublished job is refused",
            command: &["build", "--select", "job:weekly"],
            expected_plan: Err("unknown job name 'weekly'"),
        },
        AdhocPlanTestCase {
            description: "upstream selector reaches the source and seed it reads",
            command: &["build", "--select", "+orders"],
            expected_plan: Ok((
                &["model:orders", "seed:currencies", "source:shop.raw_orders"],
                json!({}),
            )),
        },
        AdhocPlanTestCase {
            description: "positional selectors join --select, and --exclude removes",
            command: &[
                "build",
                "tag:daily",
                "-s",
                "customers",
                "--exclude",
                "order_totals",
            ],
            expected_plan: Ok((&["model:customers", "model:orders"], json!({}))),
        },
        AdhocPlanTestCase {
            description: "no selector builds every node, as on the command line",
            command: &["build", "--fail-fast"],
            expected_plan: Ok((
                &[
                    "model:customers",
                    "model:order_totals",
                    "model:orders",
                    "seed:currencies",
                    "source:shop.raw_orders",
                ],
                json!({"fail_fast": true}),
            )),
        },
        AdhocPlanTestCase {
            description: "--changed with its counts and reach is the job setting a schedule would use",
            command: &[
                "build",
                "-s",
                "orders",
                "--changed=source_data,definition",
                "--reach",
                "all",
            ],
            expected_plan: Ok((
                &["model:orders"],
                json!({"changed": ["definition", "source_data"], "reach": "all"}),
            )),
        },
        AdhocPlanTestCase {
            description: "a bare --changed uses the project's [changes] defaults",
            command: &["build", "-s", "orders", "--changed"],
            expected_plan: Ok((&["model:orders"], json!({"changed": true}))),
        },
        AdhocPlanTestCase {
            description: "an abbreviated flag is the same flag, so it is still detected",
            command: &["build", "-s", "orders", "--full-ref"],
            expected_plan: Ok((&["model:orders"], json!({"full_refresh": true}))),
        },
        AdhocPlanTestCase {
            description: "store-false flags and gated flags become settings by destination",
            command: &[
                "build",
                "-s",
                "customers",
                "--no-tests",
                "--no-load",
                "--warehouse",
                "BUILD_XS",
                "--concurrency",
                "4",
            ],
            expected_plan: Ok((
                &["model:customers"],
                json!({"concurrency": 4, "load_sources": false, "run_tests": false, "warehouse": "BUILD_XS"}),
            )),
        },
        AdhocPlanTestCase {
            description: "cursor flags pass through for the server to gate",
            command: &[
                "build",
                "-s",
                "customers",
                "--start-cursor-int",
                "1",
                "--end-cursor-int",
                "9",
            ],
            expected_plan: Ok((
                &["model:customers"],
                json!({"end_cursor_int": "9", "start_cursor_int": "1"}),
            )),
        },
        AdhocPlanTestCase {
            description: "path selectors use the published folder",
            command: &["build", "-s", "path:models/staging"],
            expected_plan: Ok((&["model:customers"], json!({}))),
        },
        AdhocPlanTestCase {
            description: "variables are refused",
            command: &["build", "--vars", "{\"day\": 1}"],
            expected_plan: Err("--vars is not available for ad-hoc runs"),
        },
        AdhocPlanTestCase {
            description: "a selector file is refused",
            command: &["build", "--select-file", "selection.txt"],
            expected_plan: Err("--select-file is not available"),
        },
        AdhocPlanTestCase {
            description: "loading sources is refused",
            command: &["build", "--load"],
            expected_plan: Err("--load is not available"),
        },
        AdhocPlanTestCase {
            description: "another target is refused",
            command: &["build", "--target", "production"],
            expected_plan: Err("--target is not available"),
        },
        AdhocPlanTestCase {
            description: "debug output would print executed SQL",
            command: &["build", "--debug"],
            expected_plan: Err("--debug is not available"),
        },
        AdhocPlanTestCase {
            description: "deferral reads another target",
            command: &["build", "--defer-to", "production"],
            expected_plan: Err("--defer-to is not available"),
        },
        AdhocPlanTestCase {
            description: "a missing migration origin can lose data",
            command: &["build", "--allow-missing-migration-origin"],
            expected_plan: Err("--allow-missing-migration-origin is not available"),
        },
        AdhocPlanTestCase {
            description: "an unknown flag fails the CLI's own parse",
            command: &["build", "--everything"],
            expected_plan: Err("unrecognized arguments: --everything"),
        },
        AdhocPlanTestCase {
            description: "concurrency must be positive",
            command: &["build", "--concurrency", "0"],
            expected_plan: Err("--concurrency must be >= 1"),
        },
        AdhocPlanTestCase {
            description: "the build parser's own cursor rules still apply",
            command: &["build", "--start-cursor", "1", "--start-cursor-int", "1"],
            expected_plan: Err("cannot be combined with the -ts or -int cursor flags"),
        },
        AdhocPlanTestCase {
            description: "an unknown name is the CLI's selector error",
            command: &["build", "-s", "returns"],
            expected_plan: Err("returns"),
        },
        AdhocPlanTestCase {
            description: "loads are not ad-hoc runs yet",
            command: &["load"],
            expected_plan: Err("sqb load is not available as an ad-hoc run"),
        },
        AdhocPlanTestCase {
            description: "commands that are not runs are refused",
            command: &["server"],
            expected_plan: Err("sqb server is not available as an ad-hoc run"),
        },
        AdhocPlanTestCase {
            description: "other verbs plan with their own parser",
            command: &["test", "-s", "orders", "--case", "happy_path"],
            expected_plan: Ok((&["model:orders"], json!({"case": "happy_path"}))),
        },
        AdhocPlanTestCase {
            description: "audit settings come from the audit parser",
            command: &["audit", "-s", "orders", "--concurrency", "2"],
            expected_plan: Ok((&["model:orders"], json!({"concurrency": 2}))),
        },
        AdhocPlanTestCase {
            description: "seed runs only the seeds its selectors reach",
            command: &["seed", "+orders"],
            expected_plan: Ok((&["seed:currencies"], json!({}))),
        },
        AdhocPlanTestCase {
            description: "seed refuses a selection without seeds, which would run nothing",
            command: &["seed", "orders"],
            expected_plan: Err("the selectors match no seeds"),
        },
        AdhocPlanTestCase {
            description: "freshness observes every source upstream of a selected model",
            command: &["freshness", "order_totals", "--state", "--fail-on-stale"],
            expected_plan: Ok((
                &["source:shop.raw_orders"],
                json!({"fail_on_stale": true, "state": true}),
            )),
        },
        AdhocPlanTestCase {
            description: "freshness excludes the sources an excluded model reads",
            command: &["freshness", "--exclude", "orders"],
            expected_plan: Err("no sources are selected or upstream of the selection"),
        },
    ];
    let planner = adhoc_planner();
    for test_case in test_cases {
        let command: Vec<String> = test_case
            .command
            .iter()
            .map(|token| (*token).to_owned())
            .collect();
        let actual = planner
            .plan(&command, (&resources, &jobs))
            .map(|plan| {
                (
                    plan.selection.into_iter().collect::<Vec<String>>(),
                    json!(plan.settings),
                )
            })
            .map_err(|error| {
                test_case
                    .expected_plan
                    .as_ref()
                    .err()
                    .filter(|fragment| error.contains(**fragment))
                    .map_or(error.clone(), |fragment| (*fragment).to_owned())
            });
        assert_eq!(
            actual,
            owned_plan(&test_case.expected_plan),
            "{}",
            test_case.description
        );
    }
}
