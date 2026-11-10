use crate::authorization::main::bootstrap_user::bootstrap_user;
use crate::credentials::main::identities::create_principal::create_principal;
use crate::credentials::models::AuditContext;
use crate::platform::main::read_installation::read_installation;
use crate::platform::models::StorageKind;
use crate::platform::tests::test_types::InstallationTestCase;
use crate::store::errors::ControlError;
use crate::store::main::migrate_store::migrate_store;
use crate::store::tests::store_helpers::{close_test_store, open_test_store, run};
use crate::store::tests::store_test_types::Backend;
use uuid::Uuid;

#[test]
fn given_a_migrated_store_when_people_read_the_installation_then_only_administrators_see_it() {
    let test_cases = [
        InstallationTestCase {
            description: "SQLite",
            backend: Backend::Sqlite,
            expected_storage: StorageKind::Sqlite,
        },
        InstallationTestCase {
            description: "PostgreSQL",
            backend: Backend::Postgres,
            expected_storage: StorageKind::Postgresql,
        },
    ];
    for test_case in test_cases {
        run(async {
            let Some(fixture) = open_test_store(test_case.backend).await else {
                return;
            };
            let store = &fixture.store;
            migrate_store(store)
                .await
                .expect("migrate installation store");
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
            let installation = read_installation(store, (&context, admin))
                .await
                .expect("administrator reads the installation");
            assert_eq!(
                (
                    installation.storage,
                    installation.schema_checksum.len(),
                    installation.schema_applied_at == installation.installed_at
                ),
                (test_case.expected_storage, 64, true),
                "{}: the storage, a SHA-256 schema digest, and a schema created with the installation",
                test_case.description
            );
            assert!(
                matches!(
                    read_installation(store, (&context, developer)).await,
                    Err(ControlError::Denied { .. })
                ),
                "{}: a person without user.manage everywhere is refused",
                test_case.description
            );
            close_test_store(fixture).await;
        });
    }
}
