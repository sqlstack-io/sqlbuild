use crate::janitor_planning::models::ArchiveTimestamp;
use crate::review_command::main::parse_downstream_policy::parse_downstream_policy;
use crate::review_command::main::parse_review_schema_name::parse_review_schema_name;
use crate::review_command::main::plan_review::plan_review;
use crate::review_command::main::review_schema_name::review_schema_name;
use crate::review_command::models::{ChangedNode, DownstreamPolicy, ReviewPlan};
use crate::review_command::tests::helpers::{changed, token, waffle_shop};
use crate::review_command::tests::test_types::{
    HooksTestCase, PlanErrorTestCase, PlanTestCase, PolicyTestCase, SchemaNameBuildTestCase,
    SchemaNameTestCase,
};

const EXPORT_TASK: (&str, &str) = ("asset:export_revenue", "Python task, not run by review");
const ARCHIVE_LEFT_OUT: (&str, &str) = (
    "order_archive",
    "not on a path to a critical model (--downstream critical)",
);
const TOPPINGS_READER: &str = "reads Python node loader:raw_toppings, not rebuilt by review";

#[test]
fn given_changed_models_when_planning_then_validated_set_follows_policy_and_selection() {
    let test_cases = [
        PlanTestCase {
            description: "critical builds only the path to the critical model",
            changed: vec!["stg_orders"],
            policy: DownstreamPolicy::Critical,
            select: None,
            expected_validated: vec!["stg_orders", "order_totals", "daily_revenue"],
            expected_not_validated: vec![ARCHIVE_LEFT_OUT, EXPORT_TASK],
            expected_added: vec![],
        },
        PlanTestCase {
            description: "all builds every downstream SQL node and lists the Python node",
            changed: vec!["stg_orders"],
            policy: DownstreamPolicy::All,
            select: None,
            expected_validated: vec![
                "stg_orders",
                "order_archive",
                "order_totals",
                "daily_revenue",
            ],
            expected_not_validated: vec![EXPORT_TASK],
            expected_added: vec![],
        },
        PlanTestCase {
            description: "none builds only the changed model and lists its downstream",
            changed: vec!["stg_orders"],
            policy: DownstreamPolicy::None,
            select: None,
            expected_validated: vec!["stg_orders"],
            expected_not_validated: vec![
                (
                    "order_archive",
                    "downstream of a change (--downstream none)",
                ),
                ("order_totals", "downstream of a change (--downstream none)"),
                (
                    "daily_revenue",
                    "downstream of a change (--downstream none)",
                ),
                EXPORT_TASK,
            ],
            expected_added: vec![],
        },
        PlanTestCase {
            description: "a changed critical model needs no downstream path",
            changed: vec!["daily_revenue"],
            policy: DownstreamPolicy::Critical,
            select: None,
            expected_validated: vec!["daily_revenue"],
            expected_not_validated: vec![EXPORT_TASK],
            expected_added: vec![],
        },
        PlanTestCase {
            description: "a change with no critical downstream builds only itself",
            changed: vec!["stg_customers"],
            policy: DownstreamPolicy::Critical,
            select: None,
            expected_validated: vec!["stg_customers"],
            expected_not_validated: vec![(
                "customer_lifetime",
                "not on a path to a critical model (--downstream critical)",
            )],
            expected_added: vec![],
        },
        PlanTestCase {
            description: "select intersects with the planned scope",
            changed: vec!["stg_orders", "stg_customers"],
            policy: DownstreamPolicy::Critical,
            select: Some(vec![token("stg_orders", &["stg_orders"])]),
            expected_validated: vec!["stg_orders"],
            expected_not_validated: vec![
                ("stg_customers", "not selected (--select)"),
                (
                    "customer_lifetime",
                    "not on a path to a critical model (--downstream critical)",
                ),
                ARCHIVE_LEFT_OUT,
                ("order_totals", "not selected (--select)"),
                ("daily_revenue", "not selected (--select)"),
                EXPORT_TASK,
            ],
            expected_added: vec![],
        },
        PlanTestCase {
            description: "a graph selector keeps only its nodes inside the scope",
            changed: vec!["stg_orders"],
            policy: DownstreamPolicy::Critical,
            select: Some(vec![token(
                "stg_orders+",
                &[
                    "stg_orders",
                    "order_totals",
                    "daily_revenue",
                    "order_archive",
                ],
            )]),
            expected_validated: vec!["stg_orders", "order_totals", "daily_revenue"],
            expected_not_validated: vec![ARCHIVE_LEFT_OUT, EXPORT_TASK],
            expected_added: vec![],
        },
        PlanTestCase {
            description: "a selected downstream node adds its changed ancestor",
            changed: vec!["stg_orders"],
            policy: DownstreamPolicy::Critical,
            select: Some(vec![token("order_totals", &["order_totals"])]),
            expected_validated: vec!["stg_orders", "order_totals"],
            expected_not_validated: vec![
                ARCHIVE_LEFT_OUT,
                ("daily_revenue", "not selected (--select)"),
                EXPORT_TASK,
            ],
            expected_added: vec![("stg_orders", "order_totals")],
        },
        PlanTestCase {
            description: "a selected critical node adds every node between it and the change",
            changed: vec!["stg_orders"],
            policy: DownstreamPolicy::Critical,
            select: Some(vec![token("daily_revenue", &["daily_revenue"])]),
            expected_validated: vec!["stg_orders", "order_totals", "daily_revenue"],
            expected_not_validated: vec![ARCHIVE_LEFT_OUT, EXPORT_TASK],
            expected_added: vec![
                ("stg_orders", "daily_revenue"),
                ("order_totals", "daily_revenue"),
            ],
        },
        PlanTestCase {
            description: "a changed loader leaves it and every SQL node reading it unvalidated",
            changed: vec!["loader:raw_toppings"],
            policy: DownstreamPolicy::Critical,
            select: None,
            expected_validated: vec![],
            expected_not_validated: vec![
                ("loader:raw_toppings", "Python loader, not run by review"),
                ("stg_toppings", TOPPINGS_READER),
                ("topping_totals", TOPPINGS_READER),
            ],
            expected_added: vec![],
        },
        PlanTestCase {
            description: "an unchanged loader does not stop its readers being validated",
            changed: vec!["stg_toppings"],
            policy: DownstreamPolicy::Critical,
            select: None,
            expected_validated: vec!["stg_toppings", "topping_totals"],
            expected_not_validated: vec![],
            expected_added: vec![],
        },
    ];
    for test_case in test_cases {
        let changed_nodes: Vec<ChangedNode> = test_case
            .changed
            .iter()
            .map(|name| changed(name, "model SQL changed"))
            .collect();

        let plan: ReviewPlan = plan_review(
            &waffle_shop(),
            &changed_nodes,
            test_case.policy,
            test_case.select.as_deref(),
        )
        .expect(test_case.description);

        assert_eq!(
            plan.validated, test_case.expected_validated,
            "{}",
            test_case.description
        );
        let not_validated: Vec<(&str, &str)> = plan
            .not_validated
            .iter()
            .map(|node| (node.name.as_str(), node.reason.as_str()))
            .collect();
        assert_eq!(
            not_validated, test_case.expected_not_validated,
            "{}",
            test_case.description
        );
        let added: Vec<(&str, &str)> = plan
            .added
            .iter()
            .map(|node| (node.name.as_str(), node.required_by.as_str()))
            .collect();
        assert_eq!(added, test_case.expected_added, "{}", test_case.description);
    }
}

#[test]
fn given_validated_node_with_hooks_when_planning_then_plan_lists_it() {
    let test_cases = [
        HooksTestCase {
            description: "a validated model with hooks is listed",
            changed: vec!["stg_orders"],
            expected_hooks: vec!["order_totals"],
        },
        HooksTestCase {
            description: "a model with hooks the review leaves out is not listed",
            changed: vec!["stg_customers"],
            expected_hooks: vec![],
        },
    ];
    for test_case in test_cases {
        let changed_nodes: Vec<ChangedNode> = test_case
            .changed
            .iter()
            .map(|name| changed(name, "model SQL changed"))
            .collect();

        let plan: ReviewPlan = plan_review(
            &waffle_shop(),
            &changed_nodes,
            DownstreamPolicy::Critical,
            None,
        )
        .expect(test_case.description);

        assert_eq!(
            plan.hooks, test_case.expected_hooks,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_bad_selection_when_planning_then_fails_with_code() {
    let test_cases = [
        PlanErrorTestCase {
            description: "an unknown node",
            policy: DownstreamPolicy::Critical,
            select: vec![token("missing_model", &[])],
            expected_code: "C761",
            expected_fragment: "matches no model or seed",
        },
        PlanErrorTestCase {
            description: "an unchanged node",
            policy: DownstreamPolicy::Critical,
            select: vec![token("stg_customers", &["stg_customers"])],
            expected_code: "C762",
            expected_fragment: "did not change",
        },
        PlanErrorTestCase {
            description: "a downstream node outside the policy",
            policy: DownstreamPolicy::Critical,
            select: vec![token("order_archive", &["order_archive"])],
            expected_code: "C763",
            expected_fragment: "--downstream critical leaves out",
        },
        PlanErrorTestCase {
            description: "a node that reads a changed Python node",
            policy: DownstreamPolicy::All,
            select: vec![token("stg_toppings", &["stg_toppings"])],
            expected_code: "C763",
            expected_fragment: "read Python nodes",
        },
        PlanErrorTestCase {
            description: "any downstream node under none",
            policy: DownstreamPolicy::None,
            select: vec![token("order_totals", &["order_totals"])],
            expected_code: "C763",
            expected_fragment: "--downstream none leaves out",
        },
    ];
    for test_case in test_cases {
        let error = plan_review(
            &waffle_shop(),
            &[
                changed("stg_orders", "model SQL changed"),
                changed("loader:raw_toppings", "changed"),
            ],
            test_case.policy,
            Some(&test_case.select),
        )
        .expect_err(test_case.description);

        assert_eq!(
            error.code, test_case.expected_code,
            "{}",
            test_case.description
        );
        assert!(
            error.message.contains(test_case.expected_fragment),
            "{}: {}",
            test_case.description,
            error.message
        );
    }
}

#[test]
fn given_policy_text_when_parsing_then_known_values_map() {
    let test_cases = [
        PolicyTestCase {
            description: "critical",
            text: "critical",
            expected_policy: Some(DownstreamPolicy::Critical),
        },
        PolicyTestCase {
            description: "all",
            text: "all",
            expected_policy: Some(DownstreamPolicy::All),
        },
        PolicyTestCase {
            description: "none",
            text: "none",
            expected_policy: Some(DownstreamPolicy::None),
        },
        PolicyTestCase {
            description: "an unknown policy",
            text: "some",
            expected_policy: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            parse_downstream_policy(test_case.text),
            test_case.expected_policy,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_schema_name_parts_when_building_then_name_is_strict_and_lowercase_hex() {
    let test_cases = [
        SchemaNameBuildTestCase {
            description: "a long commit and run id are shortened, and the name is lowercase",
            parts: ("REVIEW", "20261023T101500Z", "3F2A9C1D88", "a1b2c3d4e5f6"),
            expected_name: "review_20261023t101500z_3f2a9c1_a1b2c3",
        },
        SchemaNameBuildTestCase {
            description: "a short commit and run id are zero-padded",
            parts: ("Review", "20261023T101500Z", "", "1f"),
            expected_name: "review_20261023t101500z_0000000_00001f",
        },
    ];
    for test_case in test_cases {
        let (prefix, timestamp, commit, run) = test_case.parts;
        let name: String = review_schema_name(prefix, timestamp, commit, run);

        assert_eq!(name, test_case.expected_name, "{}", test_case.description);
        assert!(
            parse_review_schema_name(&name).is_some(),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_schema_names_when_parsing_then_only_strict_names_parse() {
    let created = ArchiveTimestamp {
        year: 2026,
        month: 10,
        day: 23,
        hour: 10,
        minute: 15,
        second: 0,
    };
    let test_cases = [
        SchemaNameTestCase {
            description: "a built name",
            name: "REVIEW_20261023T101500Z_3f2a9c1_a1b2c3",
            expected_timestamp: Some(created),
        },
        SchemaNameTestCase {
            description: "a folded name with an underscored prefix",
            name: "ci_review_20261023t101500z_3f2a9c1_a1b2c3",
            expected_timestamp: Some(created),
        },
        SchemaNameTestCase {
            description: "a short commit",
            name: "REVIEW_20261023T101500Z_3f2a_a1b2c3",
            expected_timestamp: None,
        },
        SchemaNameTestCase {
            description: "an invalid month",
            name: "REVIEW_20261323T101500Z_3f2a9c1_a1b2c3",
            expected_timestamp: None,
        },
        SchemaNameTestCase {
            description: "an ordinary schema",
            name: "analytics_prod",
            expected_timestamp: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            parse_review_schema_name(test_case.name),
            test_case.expected_timestamp,
            "{}",
            test_case.description
        );
    }
}
