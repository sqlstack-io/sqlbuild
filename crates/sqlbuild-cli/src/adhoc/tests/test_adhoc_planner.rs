//! The planner parses with the CLI's own table, refuses every flag outside the allowlist and resolves exact keys.

use serde_json::json;
use sqlbuild_orchestration::runs::models::PlannerResource;

use crate::adhoc::main::adhoc_planner::adhoc_planner;
use crate::adhoc::tests::test_types::AdhocPlanTestCase;

fn resource(key: &str, deps: &[&str], tags: &[&str], folder: Option<&str>) -> PlannerResource {
    PlannerResource {
        key: key.into(),
        deps: deps.iter().map(|dep| (*dep).to_owned()).collect(),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        folder: folder.map(str::to_owned),
    }
}

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
    let test_cases = [
        AdhocPlanTestCase {
            description: "upstream selector reaches the source and seed it reads",
            command: &["build", "--select", "+orders"],
            expected: Ok((
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
            expected: Ok((&["model:customers", "model:orders"], json!({}))),
        },
        AdhocPlanTestCase {
            description: "no selector builds every node, as on the command line",
            command: &["build", "--fail-fast"],
            expected: Ok((
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
            description: "an abbreviated flag is the same flag, so it is still detected",
            command: &["build", "-s", "orders", "--full-ref"],
            expected: Ok((&["model:orders"], json!({"full_refresh": true}))),
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
            expected: Ok((
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
            expected: Ok((
                &["model:customers"],
                json!({"end_cursor_int": "9", "start_cursor_int": "1"}),
            )),
        },
        AdhocPlanTestCase {
            description: "path selectors use the published folder",
            command: &["build", "-s", "path:models/staging"],
            expected: Ok((&["model:customers"], json!({}))),
        },
        AdhocPlanTestCase {
            description: "variables are refused",
            command: &["build", "--vars", "{\"day\": 1}"],
            expected: Err("--vars is not available for ad-hoc runs"),
        },
        AdhocPlanTestCase {
            description: "a selector file is refused",
            command: &["build", "--select-file", "selection.txt"],
            expected: Err("--select-file is not available"),
        },
        AdhocPlanTestCase {
            description: "loading sources is refused",
            command: &["build", "--load"],
            expected: Err("--load is not available"),
        },
        AdhocPlanTestCase {
            description: "another target is refused",
            command: &["build", "--target", "production"],
            expected: Err("--target is not available"),
        },
        AdhocPlanTestCase {
            description: "debug output would print executed SQL",
            command: &["build", "--debug"],
            expected: Err("--debug is not available"),
        },
        AdhocPlanTestCase {
            description: "deferral reads another target",
            command: &["build", "--defer-to", "production"],
            expected: Err("--defer-to is not available"),
        },
        AdhocPlanTestCase {
            description: "a missing migration origin can lose data",
            command: &["build", "--allow-missing-migration-origin"],
            expected: Err("--allow-missing-migration-origin is not available"),
        },
        AdhocPlanTestCase {
            description: "an unknown flag fails the CLI's own parse",
            command: &["build", "--everything"],
            expected: Err("unrecognized arguments: --everything"),
        },
        AdhocPlanTestCase {
            description: "concurrency must be positive",
            command: &["build", "--concurrency", "0"],
            expected: Err("--concurrency must be >= 1"),
        },
        AdhocPlanTestCase {
            description: "the build parser's own cursor rules still apply",
            command: &["build", "--start-cursor", "1", "--start-cursor-int", "1"],
            expected: Err("cannot be combined with the -ts or -int cursor flags"),
        },
        AdhocPlanTestCase {
            description: "an unknown name is the CLI's selector error",
            command: &["build", "-s", "returns"],
            expected: Err("returns"),
        },
        AdhocPlanTestCase {
            description: "loads are not ad-hoc runs yet",
            command: &["load"],
            expected: Err("sqb load is not available as an ad-hoc run"),
        },
        AdhocPlanTestCase {
            description: "commands that are not runs are refused",
            command: &["server"],
            expected: Err("sqb server is not available as an ad-hoc run"),
        },
        AdhocPlanTestCase {
            description: "other verbs plan with their own parser",
            command: &["test", "-s", "orders", "--case", "happy_path"],
            expected: Ok((&["model:orders"], json!({"case": "happy_path"}))),
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
            .plan(&command, &resources)
            .map(|plan| {
                (
                    plan.selection.into_iter().collect::<Vec<String>>(),
                    json!(plan.settings),
                )
            })
            .map_err(|error| {
                test_case
                    .expected
                    .as_ref()
                    .err()
                    .filter(|fragment| error.contains(**fragment))
                    .map_or(error.clone(), |fragment| (*fragment).to_owned())
            });
        let expected = test_case
            .expected
            .map(|(keys, settings)| (keys.iter().map(|key| (*key).to_owned()).collect(), settings))
            .map_err(str::to_owned);
        assert_eq!(actual, expected, "{}", test_case.description);
    }
}
