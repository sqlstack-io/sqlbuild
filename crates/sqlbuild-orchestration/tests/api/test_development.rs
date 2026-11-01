//! Development defaults share production transactions, identities and operator facts on both stores.

use uuid::Uuid;

use crate::api::models::DevelopmentScope;
use crate::credentials::main::development_location::development_location;
use crate::credentials::models::AuditContext;
use crate::declarations::main::{
    change_trigger::change_trigger, list_triggers::list_triggers,
    pause_development_triggers::pause_development_triggers,
};
use crate::declarations::models::{TriggerChange, TriggerChangeRequest, TriggerStatus};
use crate::snapshots::main::{
    current_snapshot::current_snapshot, publish_snapshot::publish_snapshot,
};
use crate::store::main::migrate_store::migrate_store;

use crate::api::tests::helpers::{start_api_with_scope, stop_api};
use crate::api::tests::test_types::DevelopmentTestCase;
use crate::credentials::tests::helpers::location_state;
use crate::scheduler::tests::contracts::ticks::helpers::snapshot;
use crate::store::tests::fault_helpers::{install_fault, remove_fault};
use crate::store::tests::fault_types::TransactionFault;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_development_when_defaults_publish_then_identity_audit_and_trigger_pauses_commit_atomically()
 {
    let test_cases = [
        DevelopmentTestCase {
            description: "SQLite atomic development defaults",
            backend: Backend::Sqlite,
            expected_status: TriggerStatus::Paused,
        },
        DevelopmentTestCase {
            description: "PostgreSQL atomic development defaults",
            backend: Backend::Postgres,
            expected_status: TriggerStatus::Paused,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "local-development".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let fault = TransactionFault {
                description: "development audit failure",
                table: "sqb_security_audit",
                operation: "INSERT",
                condition: "",
                sqlite_body: "SELECT RAISE(ABORT,'injected development audit failure')",
                postgres_body: "RAISE EXCEPTION 'injected development audit failure'",
            };
            install_fault(store, &fault).await;
            assert!(
                development_location(store, "/synthetic/orders", "development", &context)
                    .await
                    .is_err()
            );
            assert_eq!(location_state(store).await, (0, 0));
            remove_fault(store, &fault).await;
            let location =
                development_location(store, "/synthetic/orders", "development", &context)
                    .await
                    .expect("create identity");
            assert_eq!(
                development_location(store, "/synthetic/orders", "development", &context)
                    .await
                    .expect("retained identity"),
                location
            );
            let other = development_location(store, "/synthetic/orders", "preview", &context)
                .await
                .expect("target-specific identity");
            assert_ne!(location, other);
            let api = start_api_with_scope(
                store,
                Some(DevelopmentScope {
                    location_id: location,
                    target: "development".into(),
                    triggers: false,
                }),
            )
            .await;
            let client = reqwest::Client::new();
            let url = format!(
                "{}/api/locations/{location}/targets/development/snapshots",
                api.url
            );
            let mut input = snapshot(location);
            install_fault(store, &fault).await;
            assert_eq!(
                client
                    .post(&url)
                    .json(&input)
                    .send()
                    .await
                    .expect("rejected publication")
                    .status(),
                reqwest::StatusCode::INTERNAL_SERVER_ERROR
            );
            assert!(
                current_snapshot(store, location, "development")
                    .await
                    .expect("last good snapshot")
                    .is_none()
            );
            assert!(
                list_triggers(store, location, "development")
                    .await
                    .expect("no leaked trigger")
                    .is_empty()
            );
            remove_fault(store, &fault).await;
            client
                .post(&url)
                .json(&input)
                .send()
                .await
                .expect("publication")
                .error_for_status()
                .expect("publication succeeds after rollback");
            let triggers = list_triggers(store, location, "development")
                .await
                .expect("paused projection");
            assert_eq!(triggers.len(), 1);
            assert_eq!(
                triggers[0].status, test_case.expected_status,
                "{}",
                test_case.description
            );
            change_trigger(
                store,
                &context,
                &TriggerChangeRequest {
                    event_id: Uuid::now_v7(),
                    trigger_id: triggers[0].trigger_id,
                    actor: context.actor.clone(),
                    reason: "Explicit development enable".into(),
                    change: TriggerChange::Enable,
                },
            )
            .await
            .expect("operator can enable retained triggers");
            input.generation = 2;
            let additional = input.documents["triggers"]["daily_orders"].clone();
            input
                .documents
                .get_mut("triggers")
                .expect("trigger document")["additional_orders"] = additional;
            client
                .post(&url)
                .json(&input)
                .send()
                .await
                .expect("reload")
                .error_for_status()
                .expect("atomic default pause on reload");
            let triggers = list_triggers(store, location, "development")
                .await
                .expect("reloaded triggers");
            assert_eq!(
                triggers
                    .iter()
                    .map(|trigger| (trigger.name.as_str(), trigger.status))
                    .collect::<Vec<_>>(),
                vec![
                    ("additional_orders", TriggerStatus::Paused),
                    ("daily_orders", TriggerStatus::Running)
                ]
            );
            let mut other_input = snapshot(other);
            other_input.target = "preview".into();
            publish_snapshot(store, &other_input)
                .await
                .expect("ordinary defaults stay enabled");
            pause_development_triggers(store, (location, "development"))
                .await
                .expect("restart pauses retained triggers");
            assert!(
                list_triggers(store, location, "development")
                    .await
                    .expect("restart state")
                    .iter()
                    .all(|trigger| trigger.status == TriggerStatus::Paused)
            );
            assert_eq!(
                list_triggers(store, other, "preview")
                    .await
                    .expect("other scope unchanged")[0]
                    .status,
                TriggerStatus::Running
            );
            stop_api(api).await;
            close_test_store(fixture).await;
        });
    }
}
