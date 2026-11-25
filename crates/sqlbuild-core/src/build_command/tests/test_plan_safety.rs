use crate::build_command::models::{
    ColumnMigrationFact, ModelErrorFact, ModelMigrationFact, PlanSafetyFacts, PlanSafetyGates,
    PlanWarningFact,
};
use crate::build_command::tests::helpers::{
    build_gates, described_steps, missing_origin, retention, snapshot, table_type,
};
use crate::build_command::tests::test_types::PolicyTestCase;

#[test]
fn given_plan_facts_when_applying_safety_gates_then_refuses_or_confirms_in_order() {
    let test_cases = [
        PolicyTestCase {
            description: "an empty plan has no gates",
            facts: PlanSafetyFacts::default(),
            gates: build_gates(),
            max_models: None,
            expected_steps: Vec::new(),
        },
        PolicyTestCase {
            description: "a model that can't be planned refuses before any migration, with its own error",
            facts: PlanSafetyFacts {
                model_errors: vec![ModelErrorFact {
                    code: Some("S203".to_owned()),
                    message: "model 'orders' sets full_refresh false".to_owned(),
                    help: Some("allow the rebuild".to_owned()),
                }],
                model_migrations: vec![ModelMigrationFact {
                    blocks_build: true,
                    decision: "conflict".to_owned(),
                    ..missing_origin("orders")
                }],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec!["refuse:S203:model 'orders' sets full_refresh false".to_owned()],
        },
        PolicyTestCase {
            description: "several models that can't be planned are joined under the first code",
            facts: PlanSafetyFacts {
                model_errors: vec![
                    ModelErrorFact {
                        code: Some("S203".to_owned()),
                        message: "model 'orders' sets full_refresh false".to_owned(),
                        help: None,
                    },
                    ModelErrorFact {
                        code: Some("S302".to_owned()),
                        message: "model 'customers' has empty inputs".to_owned(),
                        help: None,
                    },
                ],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "refuse:S203:model 'orders' sets full_refresh false; model 'customers' has empty inputs".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "a hidden blocking origin refuses first",
            facts: PlanSafetyFacts {
                model_migrations: vec![ModelMigrationFact {
                    blocks_build: true,
                    origin_hidden: true,
                    decision: "conflict".to_owned(),
                    ..missing_origin("orders")
                }],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "refuse:M102:model migration origin was built in this target but is not visible for 'orders' (migrate_from old_orders); the connection may lack permission to see it, or it was dropped outside SQLBuild; restore access to it, or remove migrate_from to build the model fresh deliberately (missing_migration_origin does not apply)".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "a conflict lists every blocked model",
            facts: PlanSafetyFacts {
                model_migrations: vec![
                    ModelMigrationFact {
                        blocks_build: true,
                        decision: "conflict".to_owned(),
                        ..missing_origin("orders")
                    },
                    ModelMigrationFact {
                        blocks_build: true,
                        decision: "migrate".to_owned(),
                        ..missing_origin("customers")
                    },
                ],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "refuse:M103:model migration conflict for 'orders', 'customers': the destination already exists with its own build history and no recorded migration".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "a blocked column rename uses its decision code",
            facts: PlanSafetyFacts {
                column_migrations: vec![ColumnMigrationFact {
                    model_name: "orders".to_owned(),
                    destination_column: "total".to_owned(),
                    origin_column: "amount".to_owned(),
                    decision: "still_produced".to_owned(),
                    decision_label: "still produced".to_owned(),
                    blocks_build: true,
                    ..ColumnMigrationFact::default()
                }],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "refuse:M111:column migrations are blocked for 'orders' (total migrate_from amount: still produced)".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "missing origins confirm once with sorted names, then the limit refuses",
            facts: PlanSafetyFacts {
                model_migrations: vec![missing_origin("orders"), missing_origin("customers")],
                executable_model_count: 3,
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: Some(2),
            expected_steps: vec![
                "confirm:build 2 models without their migrations:building 'customers', 'orders' without its missing migrate_from origin requires confirmation:building 'customers', 'orders' without its missing migrate_from origin cancelled".to_owned(),
                "refuse:C413:Build plan exceeds the configured model limit\n\nTarget:          dev\nSelected models: 3\nMaximum models:  2\nConfiguration:   targets.dev.execution_limits.max_models\n\nNo warehouse changes were made.".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "the allow flag skips the missing-origin confirmation",
            facts: PlanSafetyFacts {
                model_migrations: vec![missing_origin("orders")],
                ..PlanSafetyFacts::default()
            },
            gates: PlanSafetyGates {
                allow_missing_migration_origin: true,
                ..build_gates()
            },
            max_models: None,
            expected_steps: Vec::new(),
        },
        PolicyTestCase {
            description: "old-name errors join their messages",
            facts: PlanSafetyFacts {
                error_warnings: vec![
                    PlanWarningFact {
                        code: Some("P008".to_owned()),
                        message: "first".to_owned(),
                    },
                    PlanWarningFact {
                        code: Some("P001".to_owned()),
                        message: "unrelated".to_owned(),
                    },
                    PlanWarningFact {
                        code: Some("M114".to_owned()),
                        message: "second".to_owned(),
                    },
                ],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec!["refuse:P008:first; second".to_owned()],
        },
        PolicyTestCase {
            description: "a current-state snapshot is denied by the project policy",
            facts: PlanSafetyFacts {
                snapshot_refreshes: vec![snapshot("current", false, None)],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "refuse:C238:full refresh is denied for snapshot model 'current' by snapshot_full_refresh policy".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "the stricter model policy wins and storage gates follow in order",
            facts: PlanSafetyFacts {
                snapshot_refreshes: vec![
                    snapshot("history", true, Some("allow")),
                    snapshot("other", false, Some("require_confirmation")),
                ],
                table_type_downgrades: vec![table_type("orders", "require_confirmation")],
                retention_decreases: vec![
                    retention(&["orders", "customers"], "require_confirmation"),
                    retention(&["orders"], "require_confirmation"),
                ],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "refuse:C238:full refresh is denied for snapshot model 'other' by snapshot_full_refresh policy".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "confirmations for snapshot, table type and retention",
            facts: PlanSafetyFacts {
                snapshot_refreshes: vec![snapshot("history", true, None)],
                table_type_downgrades: vec![
                    table_type("orders", "require_confirmation"),
                    table_type("customers", "allow"),
                ],
                retention_decreases: vec![
                    retention(&["orders", "customers"], "require_confirmation"),
                    retention(&["orders"], "require_confirmation"),
                ],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "confirm:discard snapshot history for history:snapshot full refresh requires confirmation:snapshot full refresh cancelled".to_owned(),
                "confirm:downgrade table type for orders:table-type downgrade requires confirmation:table-type downgrade cancelled".to_owned(),
                "confirm:decrease retention for 2 models:time travel retention decrease requires confirmation:time travel retention decrease cancelled".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "allow flags skip storage confirmations but not denials",
            facts: PlanSafetyFacts {
                snapshot_refreshes: vec![snapshot("history", true, None)],
                table_type_downgrades: vec![table_type("orders", "require_confirmation")],
                retention_decreases: vec![retention(&["orders"], "deny")],
                ..PlanSafetyFacts::default()
            },
            gates: PlanSafetyGates {
                allow_snapshot_full_refresh: true,
                allow_table_type_downgrade: true,
                allow_retention_decrease: true,
                ..build_gates()
            },
            max_models: None,
            expected_steps: vec![
                "refuse::time travel retention decrease is denied for 'orders'".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "a denied table type refuses without a code",
            facts: PlanSafetyFacts {
                table_type_downgrades: vec![table_type("orders", "deny"), table_type("customers", "deny")],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "refuse::table-type downgrade is denied for 'orders', 'customers' by table_type_downgrade policy".to_owned(),
            ],
        },
        PolicyTestCase {
            description: "an unknown snapshot policy is an error",
            facts: PlanSafetyFacts {
                snapshot_refreshes: vec![snapshot("history", true, Some("sometimes"))],
                ..PlanSafetyFacts::default()
            },
            gates: build_gates(),
            max_models: None,
            expected_steps: vec![
                "error:'sometimes' is not a valid SnapshotFullRefreshPolicy".to_owned(),
            ],
        },
    ];

    for test_case in test_cases {
        assert_eq!(
            described_steps(&test_case.facts, &test_case.gates, test_case.max_models),
            test_case.expected_steps,
            "{}",
            test_case.description
        );
    }
}
