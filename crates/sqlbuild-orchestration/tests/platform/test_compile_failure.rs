use uuid::Uuid;

use crate::credentials::main::locations::create_location::create_location;
use crate::credentials::models::AuditContext;
use crate::platform::main::health::report_location_health::report_location_health;
use crate::platform::models::{
    LocationHealth, LocationHealthReport, LocationRuleFailure, LocationRuleFinding,
};
use crate::platform::tests::test_types::CompileFailureTestCase;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_rule_findings_when_a_location_reports_a_failed_load_then_only_bounded_lists_are_accepted()
{
    let test_cases = [
        CompileFailureTestCase {
            description: "one finding",
            finding_count: 1,
            listed: 1,
            message_chars: 60,
            expected_accepted: true,
        },
        CompileFailureTestCase {
            description: "the first 50 of many findings",
            finding_count: 222,
            listed: 50,
            message_chars: 500,
            expected_accepted: true,
        },
        CompileFailureTestCase {
            description: "more than 50 listed findings",
            finding_count: 51,
            listed: 51,
            message_chars: 60,
            expected_accepted: false,
        },
        CompileFailureTestCase {
            description: "a count below the listed findings",
            finding_count: 1,
            listed: 2,
            message_chars: 60,
            expected_accepted: false,
        },
        CompileFailureTestCase {
            description: "an over-long message",
            finding_count: 1,
            listed: 1,
            message_chars: 501,
            expected_accepted: false,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(Backend::Sqlite).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let location_id = create_location(store, "orders", &context)
                .await
                .expect("location");
            let finding = LocationRuleFinding {
                code: "SQBRMODEL102".into(),
                message: "m".repeat(test_case.message_chars),
                path: Some("models/orders.sql".into()),
                line: Some(2),
            };
            let report = LocationHealthReport {
                event_id: Uuid::now_v7(),
                session_id: Uuid::now_v7(),
                target: "development".into(),
                health: LocationHealth::CompileFailed {
                    generation: 1,
                    rules: Some(LocationRuleFailure {
                        finding_count: test_case.finding_count,
                        findings: vec![finding; test_case.listed],
                    }),
                },
            };
            assert_eq!(
                report_location_health(store, location_id, &report)
                    .await
                    .is_ok(),
                test_case.expected_accepted,
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
