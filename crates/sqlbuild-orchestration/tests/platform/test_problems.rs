use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, bootstrap_user::bootstrap_user,
};
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::{AuditContext, BuiltinRole, DefaultRole, RoleAssignmentRequest};
use crate::platform::main::read_person_problems::read_person_problems;
use crate::platform::main::report_location_health::report_location_health;
use crate::platform::models::{LocationHealth, LocationHealthReport};
use crate::scheduler::tests::contracts::helpers::acquire_lease;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::main::begin::begin;
use crate::store::main::commit::commit;
use crate::store::main::migrate_store::migrate_store;
use crate::store::main::query::query;
use crate::store::models::{Parameter, TransactionMode};
use chrono::TimeDelta;
use uuid::Uuid;

use crate::credentials::tests::location_helpers::create_location;
use crate::platform::tests::helpers::snapshot;
use crate::platform::tests::test_types::ProblemsTestCase;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_broken_locations_and_late_ticks_when_people_read_problems_then_only_listed_targets_appear()
{
    let test_cases = [
        ProblemsTestCase {
            description: "SQLite without a leader",
            backend: Backend::Sqlite,
            leaders: 0,
            expected_leader: false,
        },
        ProblemsTestCase {
            description: "PostgreSQL with a leader",
            backend: Backend::Postgres,
            leaders: 1,
            expected_leader: true,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate problems store");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: None,
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("administrator");
            let developer = create_principal(store, &context, "orders-developer")
                .await
                .expect("developer");
            let outsider = create_principal(store, &context, "orders-outsider")
                .await
                .expect("outsider");
            let mut locations = Vec::new();
            for name in ["orders", "inventory"] {
                let location = create_location(store, name, "test-admin")
                    .await
                    .expect("location");
                publish_snapshot(store, &snapshot(location))
                    .await
                    .expect("published triggers");
                report_location_health(
                    store,
                    location,
                    &LocationHealthReport {
                        event_id: Uuid::now_v7(),
                        session_id: Uuid::now_v7(),
                        target: "development".into(),
                        health: LocationHealth::Heartbeat {},
                    },
                )
                .await
                .expect("instance check-in");
                locations.push(location);
            }
            let transaction = begin(store, TransactionMode::Write)
                .await
                .expect("age check-ins");
            let stale = transaction.now - TimeDelta::minutes(5);
            let (transaction, _) = query(
                transaction,
                "UPDATE sqb_location_instances SET last_seen_at=$1",
                vec![Parameter::Time(stale)],
            )
            .await
            .expect("stale check-ins");
            commit(transaction).await.expect("aged check-ins");
            assign_builtin_role(
                store,
                (&context, admin),
                &RoleAssignmentRequest {
                    principal_id: developer,
                    role: BuiltinRole::Developer.into(),
                    location_id: Some(locations[1]),
                    target: Some("development".into()),
                    revoke: false,
                },
            )
            .await
            .expect("developer on inventory only");
            for _ in 0..test_case.leaders {
                acquire_lease(store, Uuid::now_v7())
                    .await
                    .expect("acquire lease")
                    .expect("leader");
            }
            let read = |principal: Uuid| read_person_problems(store, principal, DefaultRole::None);
            let everything = read(admin).await.expect("administrator problems");
            assert_eq!(
                everything.scheduler_leader, test_case.expected_leader,
                "{}",
                test_case.description
            );
            assert_eq!(
                everything.scheduler_lease_expires_at.is_some(),
                test_case.expected_leader
            );
            assert_eq!(
                everything
                    .broken_locations
                    .iter()
                    .map(|location| location.location_name.as_str())
                    .collect::<std::collections::BTreeSet<_>>(),
                std::collections::BTreeSet::from(["inventory", "orders"])
            );
            assert_eq!(
                everything.overdue_ticks.len(),
                200,
                "each listed target keeps its own 100 late ticks"
            );
            let scoped = read(developer).await.expect("developer problems");
            assert_eq!(scoped.scheduler_leader, everything.scheduler_leader);
            assert_eq!(
                scoped
                    .broken_locations
                    .iter()
                    .map(|location| location.location_id)
                    .collect::<Vec<_>>(),
                vec![locations[1]],
                "only the listed location is reported broken"
            );
            assert_eq!(scoped.overdue_ticks.len(), 100);
            assert!(
                scoped
                    .overdue_ticks
                    .iter()
                    .all(|tick| tick.location_id == locations[1]
                        && tick.location_name == "inventory"),
                "another location's late ticks stay hidden"
            );
            let hidden = read(outsider).await.expect("outsider problems");
            assert_eq!(hidden.scheduler_leader, everything.scheduler_leader);
            assert!(hidden.broken_locations.is_empty() && hidden.overdue_ticks.is_empty());
            close_test_store(fixture).await;
        });
    }
}
