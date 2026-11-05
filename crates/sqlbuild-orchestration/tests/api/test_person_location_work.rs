//! Person connection tests use live built-ins and the same atomic work queue on both stores.

use uuid::Uuid;

use crate::api::tests::test_types::LocationWorkTestCase;
use crate::authorization::main::{
    assign_builtin_role::assign_builtin_role, bootstrap_user::bootstrap_user,
};
use crate::credentials::main::{
    identities::create_principal::create_principal, sessions::login_password::login_password,
    sessions::set_password::set_password,
};
use crate::credentials::models::{
    AuditContext, BuiltinRole, PasswordAuthority, PasswordLogin, RoleAssignmentRequest,
};
use crate::credentials::tests::location_helpers::create_location;
use crate::platform::main::{
    read_person_connection_test::read_person_connection_test,
    request_location_work::request_location_work,
};
use crate::platform::models::{LocationWork, LocationWorkRequest};
use crate::runs::tests::helpers::snapshot;
use crate::snapshots::main::publish_snapshot::publish_snapshot;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;

#[test]
fn given_person_connection_tests_when_retried_and_revoked_then_live_scope_precedes_work_reads_and_writes()
 {
    let test_cases = [
        LocationWorkTestCase {
            description: "SQLite person work",
            backend: Backend::Sqlite,
            expected_generation: 1,
        },
        LocationWorkTestCase {
            description: "PostgreSQL person work",
            backend: Backend::Postgres,
            expected_generation: 1,
        },
    ];
    for test_case in test_cases {
        run(async {
            let fixture = open_test_store(test_case.backend).await.expect("store");
            let store = &fixture.store;
            migrate_store(store).await.expect("migrate");
            let context = AuditContext {
                actor: "control-store-owner".into(),
                granting_role: "control-store-owner".into(),
                request_id: Uuid::now_v7(),
                ip_address: Some(std::net::Ipv4Addr::LOCALHOST.into()),
            };
            let admin = bootstrap_user(store, &context, "orders-admin")
                .await
                .expect("bootstrap");
            let person = create_principal(store, &context, "orders-developer")
                .await
                .expect("person");
            let mut sessions = Vec::new();
            for (id, name) in [(admin, "orders-admin"), (person, "orders-developer")] {
                set_password(
                    store,
                    &context,
                    PasswordAuthority::Operator(id),
                    "orders-password-2026",
                )
                .await
                .expect("password");
                let PasswordLogin::Authenticated(session) =
                    login_password(store, &context, (name, "orders-password-2026"))
                        .await
                        .expect("login")
                else {
                    panic!("valid login")
                };
                sessions.push(session);
            }
            let credentials = sessions[1].identity.principal_id;
            let location = create_location(store, "orders", "test-owner")
                .await
                .expect("location");
            publish_snapshot(store, &snapshot(location))
                .await
                .expect("publish");
            let assignment = RoleAssignmentRequest {
                principal_id: person,
                role: BuiltinRole::Developer.into(),
                location_id: Some(location),
                target: Some("development".into()),
                revoke: false,
            };
            assign_builtin_role(
                store,
                (&context, sessions[0].identity.principal_id),
                &assignment,
            )
            .await
            .expect("assign existing builtin");
            let request = LocationWorkRequest {
                request_id: Uuid::now_v7(),
                location_id: location,
                target: "development".into(),
                reason: "Check orders connectivity".into(),
                work: LocationWork::CheckConnection {},
            };
            assert!(
                request_location_work(store, (&context, Some(credentials)), &request)
                    .await
                    .expect("authorized enqueue")
            );
            assert!(
                !request_location_work(store, (&context, Some(credentials)), &request)
                    .await
                    .expect("idempotent retry")
            );
            let result = read_person_connection_test(
                store,
                (&context, credentials),
                (location, "development", request.request_id),
            )
            .await
            .expect("authorized read")
            .expect("record");
            assert_eq!(
                result.generation, test_case.expected_generation,
                "{}",
                test_case.description
            );
            assert_eq!(result.requested_by, format!("principal:{person}"));
            assert!(result.result_json.is_none());
            for (other, target) in [(Uuid::now_v7(), "development"), (location, "preview")] {
                assert!(matches!(
                    read_person_connection_test(
                        store,
                        (&context, credentials),
                        (other, target, request.request_id)
                    )
                    .await,
                    Err(ControlError::Denied { .. })
                ));
                assert!(
                    read_person_connection_test(
                        store,
                        (&context, sessions[0].identity.principal_id),
                        (other, target, request.request_id)
                    )
                    .await
                    .expect("installation permission still filters stored scope")
                    .is_none()
                );
            }
            assign_builtin_role(
                store,
                (&context, sessions[0].identity.principal_id),
                &RoleAssignmentRequest {
                    revoke: true,
                    ..assignment
                },
            )
            .await
            .expect("live revoke");
            assert!(matches!(
                request_location_work(store, (&context, Some(credentials)), &request).await,
                Err(ControlError::Denied { .. })
            ));
            assert!(matches!(
                read_person_connection_test(
                    store,
                    (&context, credentials),
                    (location, "development", request.request_id)
                )
                .await,
                Err(ControlError::Denied { .. })
            ));
            close_test_store(fixture).await;
        });
    }
}
