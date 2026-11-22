//! A run's launch event says what started it: a container, or a worker running as a local process.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::authorization::main::people::bootstrap_user::bootstrap_user;
use crate::credentials::models::{AuditContext, DefaultRole};
use crate::credentials::tests::location_helpers::create_location;
use crate::runs::constants::RUN_LAUNCHED_EVENT;
use crate::runs::main::execution::change_run::change_run;
use crate::runs::main::execution::claim_next_run::claim_next_run;
use crate::runs::models::{RunAuthority, RunChange, RunChangeRequest};
use crate::runs::tests::helpers::{queue, request, snapshot};
use crate::runs::tests::test_types::LaunchEventTestCase;
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publication::publish_snapshot::publish_snapshot;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use crate::warehouse_history::main::read_person_run_events::read_person_run_events;

#[test]
fn given_a_launched_run_when_its_events_are_read_then_the_launch_names_a_container_or_a_local_worker()
 {
    let test_cases = [
        LaunchEventTestCase {
            description: "SQLite container",
            backend: Backend::Sqlite,
            container_id: Some("synthetic-orders-container"),
            expected_message: "Container started",
        },
        LaunchEventTestCase {
            description: "SQLite local process",
            backend: Backend::Sqlite,
            container_id: None,
            expected_message: "Worker started (local process)",
        },
        LaunchEventTestCase {
            description: "PostgreSQL local process",
            backend: Backend::Postgres,
            container_id: None,
            expected_message: "Worker started (local process)",
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let location = create_location(store, "orders", "test-admin")
                .await
                .expect("location");
            let published = publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            queue(
                store,
                &request((location, published.snapshot_id), 51, 0, &["orders"]),
            )
            .await;
            let lease = acquire_lease(store, Uuid::now_v7())
                .await
                .expect("elect")
                .expect("leader");
            let claim = claim_next_run(store, &lease, &BTreeMap::new(), None)
                .await
                .expect("claim")
                .expect("claimable");
            change_run(
                store,
                RunAuthority::Scheduler(&lease),
                &RunChangeRequest {
                    event_id: Uuid::now_v7(),
                    run_id: claim.request.run_id,
                    claim_id: Some(claim.claim_id),
                    change: RunChange::Launched {
                        container_id: test_case.container_id.map(str::to_owned),
                    },
                },
            )
            .await
            .expect("launched");
            let context = AuditContext {
                actor: "test-operator".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("administrator");
            let events = read_person_run_events(
                store,
                (&context, admin),
                ((location, "development", claim.request.run_id), (0, 1000)),
                DefaultRole::None,
            )
            .await
            .expect("events")
            .expect("run");
            let launched: Vec<Option<&str>> = events
                .iter()
                .filter(|event| event.event_type == RUN_LAUNCHED_EVENT)
                .map(|event| event.message.as_deref())
                .collect();
            assert_eq!(
                launched,
                vec![Some(test_case.expected_message)],
                "{}",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
